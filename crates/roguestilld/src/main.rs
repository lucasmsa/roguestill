mod enforce;
mod platform;
mod runtime;
mod snapshot;
mod storage;
use runtime::Runtime;

use chrono::{Datelike, Local, Timelike};
use platform::{MacOs, ProcessWatcher};
use roguestill_core::{
    budget::{day, Clock, LocalDay},
    rules,
    status::Status,
};
use std::{
    error::Error,
    fs,
    io::Write,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use storage::Storage;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
struct RealClock(Instant);
impl Clock for RealClock {
    fn monotonic_secs(&self) -> u64 {
        self.0.elapsed().as_secs()
    }
    fn local_day(&self, _: u64) -> LocalDay {
        let now = Local::now();
        LocalDay {
            ordinal: i64::from(now.date_naive().num_days_from_ce()),
            seconds: u64::from(now.num_seconds_from_midnight()),
        }
    }
}
fn main() -> Result<()> {
    let home = PathBuf::from(std::env::var("HOME")?);
    let base = home.join("Library/Application Support/roguestill");
    fs::create_dir_all(&base)?;
    fs::set_permissions(&base, fs::Permissions::from_mode(0o700))?;
    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| base.join("rules.toml"));
    if rules::ensure_default(&config_path)? {
        eprintln!("wrote default rules to {}", config_path.display());
    }
    let mut config_text = fs::read_to_string(&config_path)?;
    let mut config = rules::parse(&config_text)?;
    let socket = base.join("roguestilld.sock");
    if socket.exists() {
        if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
            return Err("roguestilld is already running".into());
        }
        fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
    let db = Storage::open(&base.join("usage.db"))?;
    db.log_config(Local::now().timestamp(), &config_text)?;
    let clock = RealClock(Instant::now());
    let saved = db.active()?;
    let mut runtimes = Vec::new();
    for (saved_day, old) in saved {
        let today = day(&clock, old.budget.values().1).0;
        let rule = if saved_day == today {
            Some(old.clone())
        } else {
            config.rule.iter().find(|r| r.name == old.name).cloned()
        };
        if let Some(rule) = rule {
            db.save_rule(day(&clock, rule.budget.values().1).0, &rule)?;
            runtimes.push(Runtime::new(rule, &clock, &db)?);
        } else {
            db.0.execute("DELETE FROM active_rules WHERE name=?1", [&old.name])?;
        }
    }
    for rule in &config.rule {
        if !runtimes.iter().any(|r| r.rule.name == rule.name) {
            db.save_rule(day(&clock, rule.budget.values().1).0, rule)?;
            runtimes.push(Runtime::new(rule.clone(), &clock, &db)?);
        }
    }
    let status = Arc::new(Mutex::new(Status {
        v: 1,
        rules: vec![],
    }));
    let shared = Arc::clone(&status);
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                    let current = shared.lock().unwrap().clone();
                    if let Err(error) = serde_json::to_writer(&mut stream, &current) {
                        eprintln!("socket: {error}");
                    }
                    let _ = stream.write_all(b"\n");
                }
                Err(error) => eprintln!("socket accept: {error}"),
            }
        }
    });
    let default_save = home.join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/users/crossover/AppData/LocalLow/Nolla_Games_Noita/save00");
    let mut watcher = MacOs;
    let mut notifier = MacOs;
    let mut blocker = MacOs;
    loop {
        let started = Instant::now();
        match fs::read_to_string(&config_path) {
            Ok(text) if text != config_text => match rules::parse(&text) {
                Ok(next) => {
                    db.log_config(Local::now().timestamp(), &text)?;
                    config = next;
                    config_text = text;
                }
                Err(error) => eprintln!("rules: {error}"),
            },
            Err(error) => eprintln!("rules: {error}"),
            _ => {}
        }
        let processes = watcher.processes()?;
        let mut statuses = Vec::new();
        let mut removed = Vec::new();
        for runtime in &mut runtimes {
            let save = config
                .noita_save_path
                .as_ref()
                .map(|s| {
                    s.strip_prefix("~/")
                        .map_or_else(|| PathBuf::from(s), |s| home.join(s))
                })
                .unwrap_or_else(|| default_save.clone());
            match runtime.poll(
                runtime::Poll {
                    clock: &clock,
                    db: &db,
                    config: &config,
                    save: &save,
                    processes: &processes,
                    wall: Local::now().timestamp(),
                },
                &mut notifier,
                &mut blocker,
            )? {
                Some(status) => statuses.push(status),
                None => removed.push(runtime.rule.name.clone()),
            }
        }
        for name in &removed {
            db.0.execute("DELETE FROM active_rules WHERE name=?1", [name])?;
        }
        runtimes.retain(|r| !removed.contains(&r.rule.name));
        for rule in &config.rule {
            if !runtimes.iter().any(|r| r.rule.name == rule.name) {
                db.save_rule(day(&clock, rule.budget.values().1).0, rule)?;
                runtimes.push(Runtime::new(rule.clone(), &clock, &db)?);
            }
        }
        let period = runtime::poll_interval(&statuses);
        *status.lock().unwrap() = Status {
            v: 1,
            rules: statuses,
        };
        loop {
            let remaining = period.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break;
            }
            let delay = runtimes
                .iter()
                .filter_map(|r| r.enforcement.next_delay(clock.monotonic_secs()))
                .min()
                .map(Duration::from_secs);
            if let Some(delay) = delay.filter(|delay| *delay < remaining) {
                thread::sleep(delay);
                let processes = watcher.processes()?;
                for runtime in &mut runtimes {
                    runtime
                        .enforcement
                        .service(clock.monotonic_secs(), &processes, &mut blocker);
                }
            } else {
                thread::sleep(remaining);
            }
        }
    }
}

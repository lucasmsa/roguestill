use limiter_core::{rules, status::Status};
use std::{error::Error, fs, io::Read, os::unix::net::UnixStream, path::PathBuf, time::Duration};
fn main() -> Result<(), Box<dyn Error>> {
    let base =
        PathBuf::from(std::env::var("HOME")?).join("Library/Application Support/run-limiter");
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("status") => {
            let mut socket = UnixStream::connect(base.join("limiterd.sock"))?;
            socket.set_read_timeout(Some(Duration::from_secs(3)))?;
            let mut text = String::new();
            socket.read_to_string(&mut text)?;
            let status: Status = serde_json::from_str(&text)?;
            if status.v != 1 {
                return Err("unsupported status version".into());
            }
            for rule in status.rules {
                println!(
                    "{}: used {}s, remaining {}s, runs today {}, degraded: {}, state: {}",
                    rule.name,
                    rule.used_secs,
                    rule.remaining_secs,
                    rule.runs_today,
                    rule.degraded,
                    rule.state
                );
            }
        }
        Some("rules") => {
            let path = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| base.join("rules.toml"));
            print!(
                "{}",
                rules::render(&rules::parse(&fs::read_to_string(path)?)?)?
            );
        }
        _ => return Err("usage: limiter status | limiter rules [path]".into()),
    }
    Ok(())
}

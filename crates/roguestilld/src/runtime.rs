use crate::{
    enforce,
    platform::{self, Blocker, Notifier, Process},
    snapshot,
    storage::Storage,
    Result,
};
use roguestill_core::{
    budget::{day, Clock, Engine, State},
    noita::{diff, Event, Snapshot},
    rules::{Config, Rule},
    status::RuleStatus,
};
use std::{path::Path, time::Duration};

pub struct Poll<'a, C> {
    pub clock: &'a C,
    pub db: &'a Storage,
    pub config: &'a Config,
    pub save: &'a Path,
    pub processes: &'a [Process],
    pub wall: i64,
}
pub fn poll_interval(statuses: &[RuleStatus]) -> Duration {
    if statuses.iter().any(|s| s.state == "warning") {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(2)
    }
}

pub struct Runtime {
    pub rule: Rule,
    engine: Engine,
    session: Option<i64>,
    snapshot: Option<Snapshot>,
    last_write: u64,
    pub enforcement: enforce::Enforcement,
}
impl Runtime {
    pub fn new(rule: Rule, clock: &impl Clock, db: &Storage) -> Result<Self> {
        let used = db
            .totals(&rule.name, day(clock, rule.budget.values().1).0)?
            .0;
        let engine = Engine::new(clock, &rule, used);
        Ok(Self {
            rule,
            engine,
            session: None,
            snapshot: None,
            last_write: clock.monotonic_secs(),
            enforcement: Default::default(),
        })
    }
}
impl Runtime {
    pub fn poll(
        &mut self,
        poll: Poll<'_, impl Clock>,
        notifier: &mut impl Notifier,
        blocker: &mut impl Blocker,
    ) -> Result<Option<RuleStatus>> {
        let Poll {
            clock,
            db,
            config,
            save,
            processes,
            wall,
        } = poll;
        if let Some(next) = config
            .rule
            .iter()
            .find(|r| r.name == self.rule.name && **r != self.rule)
            .filter(|_| !self.rule.lock_changes)
        {
            if self.rule.target.processes != next.target.processes {
                self.session = None;
                self.snapshot = None;
            }
            self.rule = next.clone();
            db.save_rule(day(clock, next.budget.values().1).0, next)?;
        }
        let mut matched: Vec<_> = processes
            .iter()
            .filter(|p| platform::matches_any(p, &self.rule.target.processes))
            .cloned()
            .collect();
        let mut running = !matched.is_empty();
        let now = clock.monotonic_secs();

        let mut degraded = false;
        let mut last_write = None;
        let mut events = Vec::new();
        if self.rule.adapter.is_some() {
            match snapshot::read(save) {
                Ok(next) => {
                    if let Some(previous) = &self.snapshot {
                        if next.last_write_time != previous.last_write_time {
                            self.last_write = now;
                        }
                        events = diff(previous, &next);
                    } else {
                        self.last_write = now;
                    }
                    self.snapshot = Some(next);
                    last_write = Some(self.last_write);
                }
                Err(_) => {
                    degraded = true;
                    self.snapshot = None;
                }
            }
        }
        self.engine.runs = db
            .totals(&self.rule.name, day(clock, self.rule.budget.values().1).0)?
            .1;
        let tick = self.engine.tick(clock, &self.rule, running, last_write);
        if tick.previous_day != self.engine.day {
            if self.session.is_some() || tick.previous_active > 0 {
                let boundary = wall - day(clock, self.rule.budget.values().1).1 as i64;
                db.session(
                    &mut self.session,
                    &self.rule,
                    tick.previous_day,
                    boundary,
                    tick.previous_active,
                )?;
            }
            self.session = None;
        }
        if self.session.is_some() || running || tick.active > 0 {
            db.session(
                &mut self.session,
                &self.rule,
                self.engine.day,
                wall,
                tick.active,
            )?;
        }
        if !running {
            self.session = None;
        }
        for event in events {
            match event {
                Event::RunEnded {
                    start,
                    playtime_secs,
                } => db.run(
                    &self.rule,
                    self.engine.day,
                    wall,
                    (&start, playtime_secs, true),
                )?,
                Event::Junk {
                    start,
                    playtime_secs,
                } => db.run(
                    &self.rule,
                    self.engine.day,
                    wall,
                    (&start, playtime_secs, false),
                )?,
                Event::RunStarted => {}
            }
        }
        let state = if tick.previous_day != self.engine.day {
            if let Some(next) = config.rule.iter().find(|r| r.name == self.rule.name) {
                if self.rule.target.processes != next.target.processes {
                    self.session = None;
                    self.snapshot = None;
                    matched = processes
                        .iter()
                        .filter(|p| platform::matches_any(p, &next.target.processes))
                        .cloned()
                        .collect();
                    running = !matched.is_empty();
                }
                self.rule = next.clone();
                db.save_rule(day(clock, next.budget.values().1).0, next)?;
                self.engine = Engine::new(
                    clock,
                    next,
                    db.totals(&next.name, day(clock, next.budget.values().1).0)?
                        .0,
                );
                self.engine.tick(clock, next, running, last_write).state
            } else {
                return Ok(None);
            }
        } else {
            tick.state
        };
        self.enforcement
            .apply((now, &state), &self.rule, &matched, notifier, blocker);
        let (used, runs_today) = db.totals(&self.rule.name, self.engine.day)?;
        Ok(Some(RuleStatus {
            name: self.rule.name.clone(),
            target: self.rule.target.processes[0].clone(),
            used_secs: used,
            remaining_secs: self.rule.budget.values().0.saturating_sub(used),
            runs_today,
            degraded,
            blocked_seq: self.enforcement.blocks,
            limit_runs: self.rule.budget.run_limit(),
            kill_in_secs: match state {
                State::Warning { kill_in } => Some(kill_in),
                _ => None,
            },
            state: serde_json::to_value(&state)?["kind"]
                .as_str()
                .unwrap()
                .into(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roguestill_core::{budget::LocalDay, rules};
    use std::{cell::Cell, io};
    struct FakeClock(Cell<u64>);
    impl Clock for FakeClock {
        fn monotonic_secs(&self) -> u64 {
            self.0.get()
        }
        fn local_day(&self, now: u64) -> LocalDay {
            LocalDay {
                ordinal: (now / 86400) as i64,
                seconds: now % 86400,
            }
        }
    }
    #[derive(Default)]
    struct FakePlatform(Vec<u32>);
    impl Notifier for FakePlatform {
        fn notify(&mut self, _: &str, _: u64) -> io::Result<()> {
            Ok(())
        }
    }
    impl Blocker for FakePlatform {
        fn terminate(&mut self, pid: u32) -> io::Result<()> {
            self.0.push(pid);
            Ok(())
        }
        fn kill(&mut self, pid: u32) -> io::Result<()> {
            self.0.push(pid);
            Ok(())
        }
    }
    fn config() -> Config {
        rules::parse(include_str!("../../../config/rules.example.toml")).unwrap()
    }
    fn poll(runtime: &mut Runtime, clock: &FakeClock, db: &Storage, config: &Config) -> RuleStatus {
        let processes = [Process {
            pid: 42,
            argv: "wine noita.exe".into(),
        }];
        runtime
            .poll(
                Poll {
                    clock,
                    db,
                    config,
                    save: Path::new("/nonexistent-roguestill-save00"),
                    processes: &processes,
                    wall: clock.0.get() as i64,
                },
                &mut FakePlatform::default(),
                &mut FakePlatform::default(),
            )
            .unwrap()
            .unwrap()
    }
    #[test]
    fn missing_save_falls_back_to_time_and_restart_restores_usage() {
        let clock = FakeClock(Cell::new(0));
        let config = config();
        let db = Storage::open(Path::new(":memory:")).unwrap();
        let mut runtime = Runtime::new(config.rule[0].clone(), &clock, &db).unwrap();
        assert!(poll(&mut runtime, &clock, &db, &config).degraded);
        clock.0.set(2400);
        assert_eq!(poll(&mut runtime, &clock, &db, &config).used_secs, 2400);
        let mut restarted = Runtime::new(config.rule[0].clone(), &clock, &db).unwrap();
        assert_eq!(
            poll(&mut restarted, &clock, &db, &config).remaining_secs,
            3000
        );
    }
    #[test]
    fn budget_edits_apply_immediately() {
        let clock = FakeClock(Cell::new(86390));
        let mut config = config();
        let db = Storage::open(Path::new(":memory:")).unwrap();
        let mut runtime = Runtime::new(config.rule[0].clone(), &clock, &db).unwrap();
        poll(&mut runtime, &clock, &db, &config);
        config.rule[0].budget = rules::Budget::DailyTime {
            limit_minutes: 120,
            reset_hour: 0,
            idle_pause_minutes: 5,
            limit_runs: None,
        };
        clock.0.set(86395);
        assert_eq!(
            poll(&mut runtime, &clock, &db, &config).remaining_secs,
            7195
        );
        assert_eq!(db.active().unwrap()[0].1.budget.values().0, 7200);
        clock.0.set(86400);
        assert_eq!(
            poll(&mut runtime, &clock, &db, &config).remaining_secs,
            7200
        );
        assert_eq!(db.totals(&runtime.rule.name, 0).unwrap().0, 10);
    }
    #[test]
    fn run_limit_blocks_after_the_last_allowed_run() {
        let clock = FakeClock(Cell::new(100));
        let mut config = config();
        config.rule[0].budget = rules::Budget::DailyTime {
            limit_minutes: 90,
            reset_hour: 0,
            idle_pause_minutes: 5,
            limit_runs: Some(1),
        };
        let db = Storage::open(Path::new(":memory:")).unwrap();
        let mut runtime = Runtime::new(config.rule[0].clone(), &clock, &db).unwrap();
        let before = poll(&mut runtime, &clock, &db, &config);
        assert_eq!((before.state.as_str(), before.limit_runs), ("ok", Some(1)));
        db.run(&runtime.rule, 0, 100, ("start", 600.0, true))
            .unwrap();
        clock.0.set(102);
        let after = poll(&mut runtime, &clock, &db, &config);
        assert_eq!((after.state.as_str(), after.runs_today), ("warning", 1));
        assert_eq!(after.kill_in_secs, Some(10));
        assert_eq!(before.kill_in_secs, None);
    }
    #[test]
    fn polls_every_second_only_during_a_warning() {
        let clock = FakeClock(Cell::new(100));
        let mut config = config();
        config.rule[0].budget = rules::Budget::DailyTime {
            limit_minutes: 90,
            reset_hour: 0,
            idle_pause_minutes: 5,
            limit_runs: Some(1),
        };
        let db = Storage::open(Path::new(":memory:")).unwrap();
        let mut runtime = Runtime::new(config.rule[0].clone(), &clock, &db).unwrap();
        let calm = poll(&mut runtime, &clock, &db, &config);
        assert_eq!(poll_interval(&[calm]), Duration::from_secs(2));
        db.run(&runtime.rule, 0, 100, ("start", 600.0, true))
            .unwrap();
        clock.0.set(102);
        let warning = poll(&mut runtime, &clock, &db, &config);
        assert_eq!(poll_interval(&[warning]), Duration::from_secs(1));
        assert_eq!(poll_interval(&[]), Duration::from_secs(2));
    }
    fn edit_limit(config: &mut Config, minutes: u64) {
        config.rule[0].budget = rules::Budget::DailyTime {
            limit_minutes: minutes,
            reset_hour: 0,
            idle_pause_minutes: 5,
            limit_runs: None,
        };
    }
    #[test]
    fn locked_rule_ignores_edits_until_reset_and_unlocking_waits_too() {
        let clock = FakeClock(Cell::new(86390));
        let mut config = config();
        let db = Storage::open(Path::new(":memory:")).unwrap();
        let mut runtime = Runtime::new(config.rule[0].clone(), &clock, &db).unwrap();
        poll(&mut runtime, &clock, &db, &config);
        config.rule[0].lock_changes = true;
        clock.0.set(86392);
        poll(&mut runtime, &clock, &db, &config);
        assert!(runtime.rule.lock_changes);
        edit_limit(&mut config, 120);
        clock.0.set(86395);
        assert_eq!(
            poll(&mut runtime, &clock, &db, &config).remaining_secs,
            5395
        );
        config.rule[0].lock_changes = false;
        clock.0.set(86396);
        assert_eq!(
            poll(&mut runtime, &clock, &db, &config).remaining_secs,
            5394
        );
        assert!(runtime.rule.lock_changes);
        clock.0.set(86400);
        assert_eq!(
            poll(&mut runtime, &clock, &db, &config).remaining_secs,
            7200
        );
        assert!(!runtime.rule.lock_changes);
    }
    #[test]
    fn group_members_share_one_budget() {
        let clock = FakeClock(Cell::new(100));
        let mut config = config();
        config.rule[0].target.processes = vec!["Noita".into(), "Hades.exe".into()];
        config.rule[0].adapter = None;
        let db = Storage::open(Path::new(":memory:")).unwrap();
        let mut runtime = Runtime::new(config.rule[0].clone(), &clock, &db).unwrap();
        let run = |argv: &str, runtime: &mut Runtime| {
            let processes = [Process {
                pid: 7,
                argv: argv.into(),
            }];
            runtime
                .poll(
                    Poll {
                        clock: &clock,
                        db: &db,
                        config: &config,
                        save: Path::new("/nonexistent"),
                        processes: &processes,
                        wall: clock.0.get() as i64,
                    },
                    &mut FakePlatform::default(),
                    &mut FakePlatform::default(),
                )
                .unwrap()
                .unwrap()
        };
        run("wine C:\\Games\\noita.exe", &mut runtime);
        clock.0.set(160);
        assert_eq!(run("wine C:\\Games\\noita.exe", &mut runtime).used_secs, 60);
        clock.0.set(200);
        assert_eq!(run("/Games/Hades.exe", &mut runtime).used_secs, 100);
        clock.0.set(230);
        assert_eq!(run("/usr/bin/other", &mut runtime).used_secs, 130);
        clock.0.set(300);
        assert_eq!(run("/usr/bin/other", &mut runtime).used_secs, 130);
    }
}

use crate::{
    enforce,
    platform::{self, Blocker, Notifier, Process},
    snapshot,
    storage::Storage,
    Result,
};
use limiter_core::{
    budget::{day, Clock, Engine},
    noita::{diff, Event, Snapshot},
    rules::{Config, Rule},
    status::RuleStatus,
};
use std::path::Path;

pub struct Poll<'a, C> {
    pub clock: &'a C,
    pub db: &'a Storage,
    pub config: &'a Config,
    pub save: &'a Path,
    pub processes: &'a [Process],
    pub wall: i64,
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
        let mut matched: Vec<_> = processes
            .iter()
            .filter(|p| platform::matches(p, &self.rule.target.process))
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
                if self.rule.target.process != next.target.process {
                    self.session = None;
                    self.snapshot = None;
                    matched = processes
                        .iter()
                        .filter(|p| platform::matches(p, &next.target.process))
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
            target: self.rule.target.process.clone(),
            used_secs: used,
            remaining_secs: self.rule.budget.values().0.saturating_sub(used),
            runs_today,
            degraded,
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
    use limiter_core::{budget::LocalDay, rules};
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
                    save: Path::new("/nonexistent-run-limiter-save00"),
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
    fn budget_edits_wait_for_reset() {
        let clock = FakeClock(Cell::new(86390));
        let mut config = config();
        let db = Storage::open(Path::new(":memory:")).unwrap();
        let mut runtime = Runtime::new(config.rule[0].clone(), &clock, &db).unwrap();
        poll(&mut runtime, &clock, &db, &config);
        config.rule[0].budget = rules::Budget::DailyTime {
            limit_minutes: 120,
            reset_hour: 0,
            idle_pause_minutes: 5,
        };
        clock.0.set(86395);
        assert_eq!(
            poll(&mut runtime, &clock, &db, &config).remaining_secs,
            5395
        );
        clock.0.set(86400);
        assert_eq!(
            poll(&mut runtime, &clock, &db, &config).remaining_secs,
            7200
        );
        assert_eq!(db.totals(&runtime.rule.name, 0).unwrap().0, 10);
        assert_eq!(db.active().unwrap()[0].1.budget.values().0, 7200);
    }
    #[test]
    fn additions_wait_for_reset() {
        let db = Storage::open(Path::new(":memory:")).unwrap();
        assert!(db.addition_ready("initial", 1, true).unwrap());
        assert!(!db.addition_ready("new", 1, false).unwrap());
        assert!(!db.addition_ready("new", 1, false).unwrap());
        assert!(db.addition_ready("new", 2, false).unwrap());
    }
}

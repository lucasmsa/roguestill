use roguestill_core::{
    budget::{Clock, Engine, LocalDay, State},
    noita::{diff, parse_stats, Event, Snapshot},
    rules::{parse, Budget, Rule},
};
use std::cell::Cell;
struct FakeClock {
    now: Cell<u64>,
    wall: Cell<u64>,
}
impl FakeClock {
    fn new(wall: u64) -> Self {
        Self {
            now: Cell::new(0),
            wall: Cell::new(wall),
        }
    }
    fn advance(&self, seconds: u64) {
        self.now.set(self.now.get() + seconds);
        self.wall.set(self.wall.get() + seconds);
    }
}
impl Clock for FakeClock {
    fn monotonic_secs(&self) -> u64 {
        self.now.get()
    }
    fn local_day(&self, _: u64) -> LocalDay {
        LocalDay {
            ordinal: (self.wall.get() / 86400) as i64,
            seconds: self.wall.get() % 86400,
        }
    }
}
fn rule() -> Rule {
    parse(include_str!("../../../config/rules.example.toml"))
        .unwrap()
        .rule
        .remove(0)
}
#[test]
fn midnight_and_configured_reset_split_usage() {
    for hour in [0, 4] {
        let mut rule = rule();
        rule.budget = Budget::DailyTime {
            limit_minutes: 90,
            reset_hour: hour,
            idle_pause_minutes: 5,
        };
        let clock = FakeClock::new(86400 + u64::from(hour) * 3600 - 10);
        let mut engine = Engine::new(&clock, &rule, 100);
        engine.tick(&clock, &rule, true, None);
        clock.advance(20);
        let tick = engine.tick(&clock, &rule, true, None);
        assert_eq!(tick.previous_active, 10);
        assert_eq!(tick.active, 10);
        assert_eq!(engine.used, 10);
        assert_eq!(tick.state, State::Ok { remaining: 5390 });
    }
}
#[test]
fn forty_minute_idle_costs_five() {
    let clock = FakeClock::new(0);
    let rule = rule();
    let mut engine = Engine::new(&clock, &rule, 0);
    engine.tick(&clock, &rule, true, Some(0));
    clock.advance(2400);
    engine.tick(&clock, &rule, true, Some(0));
    assert_eq!(engine.used, 300);
    clock.advance(2);
    engine.tick(&clock, &rule, true, Some(2402));
    assert_eq!(engine.used, 302);
}
#[test]
fn monotonic_forward_never_refunds() {
    let clock = FakeClock::new(3600);
    let rule = rule();
    let mut engine = Engine::new(&clock, &rule, 0);
    engine.tick(&clock, &rule, true, None);
    clock.now.set(1800);
    engine.tick(&clock, &rule, true, None);
    assert_eq!(engine.used, 1800);
    clock.now.set(3600);
    engine.tick(&clock, &rule, true, None);
    assert_eq!(engine.used, 3600);
}
#[test]
fn warning_grace_and_relaunch() {
    let clock = FakeClock::new(0);
    let rule = rule();
    let mut engine = Engine::new(&clock, &rule, 5390);
    engine.tick(&clock, &rule, true, None);
    clock.advance(10);
    assert_eq!(
        engine.tick(&clock, &rule, true, None).state,
        State::Warning { kill_in: 10 }
    );
    clock.advance(9);
    assert_eq!(
        engine.tick(&clock, &rule, true, None).state,
        State::Warning { kill_in: 1 }
    );
    clock.advance(1);
    assert_eq!(
        engine.tick(&clock, &rule, true, None).state,
        State::Exceeded
    );
    engine.tick(&clock, &rule, false, None);
    assert_eq!(
        engine.tick(&clock, &rule, true, None).state,
        State::Exceeded
    );
    let mut restarted = Engine::new(&clock, &rule, 5400);
    assert_eq!(
        restarted.tick(&clock, &rule, true, None).state,
        State::Exceeded
    );
}
#[test]
fn junk_threshold_and_quit_to_desktop() {
    let before = Snapshot {
        world_file_count: 12,
        ..Default::default()
    };
    for seconds in [29, 30] {
        let session = parse_stats(
            "run",
            &format!("<Stats playtime=\"{seconds}\" dead=\"1\"/>"),
        )
        .unwrap();
        let mut after = Snapshot {
            sessions: vec![session],
            ..Default::default()
        };
        let events = diff(&before, &after);
        assert_eq!(events.len(), 1);
        assert_eq!(matches!(events[0], Event::Junk { .. }), seconds == 29);
        assert_eq!(matches!(events[0], Event::RunEnded { .. }), seconds == 30);
        after.sessions.clear();
        after.world_file_count = 12;
        assert!(diff(&before, &after).is_empty());
    }
    assert_eq!(diff(&Snapshot::default(), &before), vec![Event::RunStarted]);
}
#[test]
fn invalid_stats_are_rejected() {
    for xml in [
        "<Stats/>",
        "<Stats playtime='NaN' dead='1'/>",
        "<Stats playtime='-1' dead='1'/>",
        "<Stats playtime='30' dead='x'/>",
    ] {
        assert!(parse_stats("run", xml).is_err());
    }
    assert!(
        !parse_stats("run", "<Stats playtime = '30.5' dead = '0'/>")
            .unwrap()
            .dead
    );
}
#[test]
fn strict_rules() {
    let valid = include_str!("../../../config/rules.example.toml");
    assert!(parse(valid).is_ok());
    for invalid in [
        valid.replace("limit_minutes = 90", "limit_minutes = 0"),
        valid.replace("reset_hour = 0", "reset_hour = 24"),
        valid.replace("kill = true", "kill = true\nunknown = 1"),
        valid.replace("process =", "typo ="),
        valid.replace("daily_time", "run_count"),
        valid.replace("noita\"", "unknown\""),
        valid.replace("kill_again", "allow"),
        format!("{valid}\n{valid}"),
    ] {
        assert!(parse(&invalid).is_err(), "{invalid}");
    }
}

#[test]
fn death_seen_while_new_run_already_loading() {
    let before = Snapshot {
        world_file_count: 153,
        ..Default::default()
    };
    let after = Snapshot {
        world_file_count: 3,
        sessions: vec![parse_stats(
            "20261117-080256",
            "<Stats playtime=\"1203.68\" dead=\"1\"/>",
        )
        .unwrap()],
        ..Default::default()
    };
    assert_eq!(
        diff(&before, &after),
        vec![Event::RunEnded {
            start: "20261117-080256".into(),
            playtime_secs: 1203.68
        }]
    );
}

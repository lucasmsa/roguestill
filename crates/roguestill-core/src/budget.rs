use crate::rules::Rule;
use serde::Serialize;

#[derive(Clone, Copy)]
pub struct LocalDay {
    pub ordinal: i64,
    pub seconds: u64,
}
pub trait Clock {
    fn monotonic_secs(&self) -> u64;
    fn local_day(&self, now: u64) -> LocalDay;
}

pub fn day(clock: &impl Clock, reset_hour: u8) -> (i64, u64) {
    let local = clock.local_day(clock.monotonic_secs());
    let reset = u64::from(reset_hour) * 3600;
    (
        local.ordinal - i64::from(local.seconds < reset),
        (local.seconds + 86400 - reset) % 86400,
    )
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum State {
    Ok { remaining: u64 },
    Warning { kill_in: u64 },
    Exceeded,
}

pub struct Engine {
    pub used: u64,
    pub runs: u64,
    pub day: i64,
    last: u64,
    running: bool,
    warning_at: Option<u64>,
}

pub struct Tick {
    pub state: State,
    pub previous_day: i64,
    pub previous_active: u64,
    pub active: u64,
}

impl Engine {
    pub fn new(clock: &impl Clock, rule: &Rule, used: u64) -> Self {
        Self {
            used,
            runs: 0,
            day: day(clock, rule.budget.values().1).0,
            last: clock.monotonic_secs(),
            running: false,
            warning_at: None,
        }
    }

    pub fn tick(
        &mut self,
        clock: &impl Clock,
        rule: &Rule,
        running: bool,
        last_write: Option<u64>,
    ) -> Tick {
        let now = clock.monotonic_secs().max(self.last);
        let (limit, reset, idle) = rule.budget.values();
        let (today, elapsed) = day(clock, reset);
        let end = last_write.map_or(now, |write| now.min(write.saturating_add(idle)));
        let active = if self.running {
            end.saturating_sub(self.last)
        } else {
            0
        };
        let previous_day = self.day;
        let boundary = now.saturating_sub(elapsed);
        let previous_active = if today != self.day && self.running {
            end.min(boundary).saturating_sub(self.last).min(active)
        } else {
            0
        };
        if today != self.day {
            self.used = 0;
            self.runs = 0;
            self.day = today;
            self.warning_at = None;
        }
        let active = active - previous_active;
        self.used = self.used.saturating_add(active);
        let runs_done = rule.budget.run_limit().is_some_and(|max| self.runs >= max);
        let state = if self.used < limit && !runs_done {
            State::Ok {
                remaining: limit - self.used,
            }
        } else {
            if !self.running && running {
                self.warning_at = Some(now);
            }
            let since = *self.warning_at.get_or_insert(now);
            let kill_in = rule
                .action
                .warn_seconds
                .saturating_sub(now.saturating_sub(since));
            if kill_in == 0 {
                State::Exceeded
            } else {
                State::Warning { kill_in }
            }
        };
        self.last = now;
        self.running = running;
        Tick {
            state,
            previous_day,
            previous_active,
            active,
        }
    }
}

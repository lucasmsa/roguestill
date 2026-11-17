use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct Session {
    pub run_start_name: String,
    pub playtime_secs: f64,
    pub dead: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub sessions: Vec<Session>,
    pub world_file_count: usize,
    pub last_write_time: u64,
}
#[derive(Debug, PartialEq)]
pub enum Event {
    RunEnded { start: String, playtime_secs: f64 },
    Junk { start: String, playtime_secs: f64 },
    RunStarted,
}
pub fn diff(before: &Snapshot, after: &Snapshot) -> Vec<Event> {
    let mut events = Vec::new();
    if before.world_file_count == 0 && after.world_file_count > 0 {
        events.push(Event::RunStarted);
    }
    if before.world_file_count > 0 && after.world_file_count == 0 {
        let known: HashSet<_> = before.sessions.iter().map(|s| &s.run_start_name).collect();
        for session in &after.sessions {
            if !known.contains(&session.run_start_name) {
                let start = session.run_start_name.clone();
                let playtime_secs = session.playtime_secs;
                events.push(if playtime_secs < 30.0 {
                    Event::Junk {
                        start,
                        playtime_secs,
                    }
                } else {
                    Event::RunEnded {
                        start,
                        playtime_secs,
                    }
                });
            }
        }
    }
    events
}

pub fn parse_stats(name: &str, xml: &str) -> Result<Session, String> {
    fn attribute<'a>(xml: &'a str, key: &str) -> Option<&'a str> {
        for (index, _) in xml.match_indices(key) {
            if index > 0 && !xml.as_bytes()[index - 1].is_ascii_whitespace() {
                continue;
            }
            let tail = xml[index + key.len()..]
                .trim_start()
                .strip_prefix('=')?
                .trim_start();
            let quote = tail.chars().next()?;
            if quote != '"' && quote != '\'' {
                continue;
            }
            return tail[1..].split_once(quote).map(|(value, _)| value);
        }
        None
    }
    let playtime_secs: f64 = attribute(xml, "playtime")
        .ok_or("missing playtime")?
        .parse()
        .map_err(|_| "invalid playtime")?;
    if !playtime_secs.is_finite() || playtime_secs < 0.0 {
        return Err("invalid playtime".into());
    }
    let dead = match attribute(xml, "dead") {
        Some("1") => true,
        Some("0") => false,
        _ => return Err("invalid dead".into()),
    };
    Ok(Session {
        run_start_name: name.to_owned(),
        playtime_secs,
        dead,
    })
}

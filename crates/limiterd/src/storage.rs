use limiter_core::rules::Rule;
use rusqlite::{params, Connection, OptionalExtension};

pub struct Storage(pub Connection);
impl Storage {
    pub fn open(path: &std::path::Path) -> rusqlite::Result<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch("PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS usage (
                id INTEGER PRIMARY KEY, rule TEXT NOT NULL, target TEXT NOT NULL,
                day INTEGER NOT NULL, start INTEGER NOT NULL, end INTEGER NOT NULL,
                active_secs INTEGER NOT NULL, run_id TEXT, counted INTEGER NOT NULL DEFAULT 0,
                UNIQUE(rule, run_id));
            CREATE TABLE IF NOT EXISTS active_rules (name TEXT PRIMARY KEY, day INTEGER NOT NULL, json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS pending_rules (name TEXT PRIMARY KEY, day INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS rule_edits (at INTEGER NOT NULL, config TEXT NOT NULL);")?;
        Ok(Self(connection))
    }
    pub fn initialized(&self) -> rusqlite::Result<bool> {
        self.0
            .query_row("SELECT EXISTS(SELECT 1 FROM rule_edits)", [], |row| {
                row.get(0)
            })
    }
    pub fn addition_ready(&self, name: &str, day: i64, initial: bool) -> rusqlite::Result<bool> {
        if initial {
            return Ok(true);
        }
        self.0.execute(
            "INSERT OR IGNORE INTO pending_rules VALUES (?1,?2)",
            params![name, day],
        )?;
        let seen: i64 = self.0.query_row(
            "SELECT day FROM pending_rules WHERE name=?1",
            [name],
            |row| row.get(0),
        )?;
        Ok(seen != day)
    }
    pub fn active(&self) -> rusqlite::Result<Vec<(i64, Rule)>> {
        let mut statement = self.0.prepare("SELECT day, json FROM active_rules")?;
        let rows = statement
            .query_map([], |row| {
                let json: String = row.get(1)?;
                let rule = serde_json::from_str(&json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok((row.get(0)?, rule))
            })?
            .collect();
        rows
    }
    pub fn save_rule(&self, day: i64, rule: &Rule) -> rusqlite::Result<()> {
        self.0.execute(
            "INSERT OR REPLACE INTO active_rules VALUES (?1, ?2, ?3)",
            params![
                rule.name,
                day,
                serde_json::to_string(rule).expect("rule serializes")
            ],
        )?;
        self.0
            .execute("DELETE FROM pending_rules WHERE name=?1", [&rule.name])?;
        Ok(())
    }
    pub fn totals(&self, name: &str, day: i64) -> rusqlite::Result<(u64, u64)> {
        self.0.query_row("SELECT COALESCE(SUM(CASE WHEN run_id IS NULL THEN active_secs ELSE 0 END),0), COALESCE(SUM(counted),0) FROM usage WHERE rule=?1 AND day=?2", params![name, day], |row| Ok((row.get(0)?, row.get(1)?)))
    }
    pub fn session(
        &self,
        id: &mut Option<i64>,
        rule: &Rule,
        day: i64,
        now: i64,
        active: u64,
    ) -> rusqlite::Result<()> {
        if let Some(id) = *id {
            self.0.execute(
                "UPDATE usage SET end=?1, active_secs=active_secs+?2 WHERE id=?3",
                params![now, active, id],
            )?;
        } else {
            self.0.execute("INSERT INTO usage(rule,target,day,start,end,active_secs) VALUES (?1,?2,?3,?4,?5,?6)", params![rule.name, rule.target.process, day, now - active as i64, now, active])?;
            *id = Some(self.0.last_insert_rowid());
        }
        Ok(())
    }
    pub fn run(
        &self,
        rule: &Rule,
        day: i64,
        now: i64,
        run: (&str, f64, bool),
    ) -> rusqlite::Result<()> {
        let (run, seconds, counted) = run;
        self.0.execute("INSERT OR IGNORE INTO usage(rule,target,day,start,end,active_secs,run_id,counted) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)", params![rule.name, rule.target.process, day, now - seconds as i64, now, seconds as u64, run, counted])?;
        Ok(())
    }
    pub fn log_config(&self, now: i64, config: &str) -> rusqlite::Result<()> {
        let previous: Option<String> = self
            .0
            .query_row(
                "SELECT config FROM rule_edits ORDER BY rowid DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if previous.as_deref() != Some(config) {
            self.0.execute(
                "INSERT INTO rule_edits VALUES (?1,?2)",
                params![now, config],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule() -> Rule {
        limiter_core::rules::parse(include_str!("../../../config/rules.example.toml"))
            .unwrap()
            .rule
            .remove(0)
    }
    #[test]
    fn totals_exclude_run_double_counting_and_deduplicate() {
        let db = Storage::open(std::path::Path::new(":memory:")).unwrap();
        let rule = rule();
        let mut id = None;
        db.session(&mut id, &rule, 1, 100, 60).unwrap();
        db.session(&mut id, &rule, 1, 110, 10).unwrap();
        db.run(&rule, 1, 110, ("run", 30.0, true)).unwrap();
        db.run(&rule, 1, 110, ("run", 30.0, true)).unwrap();
        db.run(&rule, 1, 110, ("junk", 29.0, false)).unwrap();
        assert_eq!(db.totals(&rule.name, 1).unwrap(), (70, 1));
        assert_eq!(db.totals(&rule.name, 2).unwrap(), (0, 0));
    }
    #[test]
    fn active_rule_and_edit_log_roundtrip() {
        let db = Storage::open(std::path::Path::new(":memory:")).unwrap();
        db.save_rule(1, &rule()).unwrap();
        assert_eq!(db.active().unwrap()[0].0, 1);
        db.log_config(0, "old").unwrap();
        db.log_config(1, "old").unwrap();
        db.log_config(2, "new").unwrap();
        let count: u64 =
            db.0.query_row("SELECT COUNT(*) FROM rule_edits", [], |row| row.get(0))
                .unwrap();
        assert_eq!(count, 2);
    }
}

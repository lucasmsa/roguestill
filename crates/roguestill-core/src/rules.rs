use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub noita_save_path: Option<String>,
    pub rule: Vec<Rule>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub name: String,
    pub target: Target,
    pub adapter: Option<Adapter>,
    pub budget: Budget,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub process: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Adapter {
    Noita,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Budget {
    DailyTime {
        limit_minutes: u64,
        #[serde(default)]
        reset_hour: u8,
        #[serde(default = "idle_default")]
        idle_pause_minutes: u64,
    },
}
fn idle_default() -> u64 {
    5
}

impl Budget {
    pub fn values(&self) -> (u64, u8, u64) {
        match *self {
            Self::DailyTime {
                limit_minutes,
                reset_hour,
                idle_pause_minutes,
            } => (limit_minutes * 60, reset_hour, idle_pause_minutes * 60),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    #[serde(default = "warn_default")]
    pub warn_seconds: u64,
    pub kill: bool,
    #[serde(default)]
    pub relaunch: Relaunch,
}
fn warn_default() -> u64 {
    10
}
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Relaunch {
    #[default]
    KillAgain,
}

pub fn parse(input: &str) -> Result<Config, String> {
    let config: Config = toml::from_str(input).map_err(|e| e.to_string())?;
    let mut names = HashSet::new();
    for rule in &config.rule {
        let Budget::DailyTime {
            limit_minutes,
            reset_hour,
            idle_pause_minutes,
        } = rule.budget;
        if rule.name.trim().is_empty()
            || rule.target.process.trim().is_empty()
            || !names.insert(&rule.name)
            || reset_hour > 23
            || limit_minutes == 0
            || limit_minutes > u64::MAX / 60
            || idle_pause_minutes > u64::MAX / 60
        {
            return Err(format!("invalid rule: {}", rule.name));
        }
    }
    Ok(config)
}

pub fn render(config: &Config) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(config)
}

const DEFAULT_RULES: &str = include_str!("../../../config/rules.example.toml");

pub fn ensure_default(path: &std::path::Path) -> std::io::Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, DEFAULT_RULES)?;
    Ok(true)
}

pub fn to_json(config: &Config) -> Result<String, String> {
    serde_json::to_string(config).map_err(|e| e.to_string())
}

pub fn from_json(input: &str) -> Result<Config, String> {
    let config: Config = serde_json::from_str(input).map_err(|e| e.to_string())?;
    parse(&render(&config).map_err(|e| e.to_string())?)
}

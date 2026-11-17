use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Status {
    pub v: u8,
    pub rules: Vec<RuleStatus>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuleStatus {
    pub name: String,
    pub target: String,
    pub used_secs: u64,
    pub remaining_secs: u64,
    pub runs_today: u64,
    pub degraded: bool,
    pub state: String,
    #[serde(default)]
    pub blocked_seq: u64,
}

use super::sessions::PowerAction;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Timer,
    Playback,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Countdown {
    pub deadline: u64,
    pub action: PowerAction,
    pub source: Source,
}

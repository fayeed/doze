use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PowerAction {
    Sleep,
    Hibernate,
    Shutdown,
    Lock,
    DisplayOff,
}

impl PowerAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sleep => "Sleep",
            Self::Hibernate => "Hibernate",
            Self::Shutdown => "Shut down",
            Self::Lock => "Lock",
            Self::DisplayOff => "Turn display off",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub launch_at_startup: bool,
    pub start_minimized: bool,
    pub notifications: bool,
    pub default_action: PowerAction,
    pub silence_seconds: u64,
    pub idle_seconds: u64,
    pub countdown_seconds: u64,
    pub playback_action: PowerAction,
    pub logging: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            launch_at_startup: false,
            start_minimized: true,
            notifications: true,
            default_action: PowerAction::Sleep,
            silence_seconds: 60,
            idle_seconds: 300,
            countdown_seconds: 300,
            playback_action: PowerAction::Sleep,
            logging: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(10..=3600).contains(&self.silence_seconds)
            || !(30..=7200).contains(&self.idle_seconds)
            || !(15..=1800).contains(&self.countdown_seconds)
        {
            return Err("Use silence 10–3600s, idle 30–7200s, and countdown 15–1800s.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Active,
    GracePeriod,
    Countdown,
    Cancelled,
    Completed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timer {
    pub deadline: u64,
    pub action: PowerAction,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Engine {
    pub now: u64,
    pub awake: bool,
    pub awake_deadline: Option<u64>,
    pub while_audio: bool,
    pub audio_active: bool,
    pub playback_enabled: bool,
    pub playback_phase: Phase,
    pub timer: Option<Timer>,
    pub countdown: Option<super::countdown::Countdown>,
    pub message: Option<String>,
    #[serde(skip)]
    pub silence_since: Option<u64>,
    #[serde(skip)]
    pub audio_streak: u8,
    #[serde(skip)]
    pub last_audio_at: Option<u64>,
}
impl Default for Engine {
    fn default() -> Self {
        Self {
            now: 0,
            awake: false,
            awake_deadline: None,
            while_audio: false,
            audio_active: false,
            playback_enabled: false,
            playback_phase: Phase::Idle,
            timer: None,
            countdown: None,
            message: None,
            silence_since: None,
            audio_streak: 0,
            last_audio_at: None,
        }
    }
}

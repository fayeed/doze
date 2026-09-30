use crate::core::sessions::{Engine, PowerAction, Settings};

use std::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub enum Operation {
    KeepAwake { seconds: Option<u64> },
    KeepAwakeDefault,
    ScheduleDefault,
    TogglePreference { preference: Preference },
    SetDefaultDuration { awake: bool, minutes: u64 },
    ExtendAwake { seconds: u64 },
    StopAwake,
    ToggleWhileAudio,
    TogglePlayback,
    SelectAction { action: PowerAction },
    ScheduleSelected { seconds: u64 },
    Schedule { seconds: u64, action: PowerAction },
    StopTimer,
    Cancel,
    Snooze,
    SaveSettings { settings: Settings },
    Refresh,
    PreviewCountdown,
    OpenDialog { view: DialogView },
    Quit,
}
#[derive(Clone, Copy, Debug)]
pub enum DialogView {
    Settings,
    Help,
    About,
    AwakeDuration,
    AwakeTime,
    TimerDuration,
    TimerTime,
}
pub enum Request {
    Operation(Operation, Sender<Result<Snapshot, String>>),
    Lifecycle,
    WarningFailed(String),
}
#[derive(Clone)]
pub struct Snapshot {
    pub settings_path: std::path::PathBuf,
    pub engine: Engine,
    pub settings: Settings,
    pub actions: Vec<PowerAction>,
    pub audio_supported: bool,
    pub startup_supported: bool,
    pub error: Option<String>,
    pub selected_action: PowerAction,
    pub view: DialogView,
}

#[derive(Clone, Copy, Debug)]
pub enum Preference {
    AllowDisplaySleep,
    Notifications,
    LaunchAtStartup,
    StartMinimized,
    Logging,
}

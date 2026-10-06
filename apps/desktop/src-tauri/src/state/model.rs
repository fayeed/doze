use crate::core::sessions::{Engine, PowerAction, Settings};

use std::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub enum Operation {
    KeepAwake {
        seconds: Option<u64>,
    },
    KeepAwakeDefault,
    ScheduleDefault,
    TogglePreference {
        preference: Preference,
    },
    SetDefaultDuration {
        awake: bool,
        minutes: u64,
    },
    ExtendAwake {
        seconds: u64,
    },
    StopAwake,
    ToggleWhileAudio,
    TogglePlayback,
    SelectAction {
        action: PowerAction,
    },
    ScheduleSelected {
        seconds: u64,
    },
    Schedule {
        seconds: u64,
        action: PowerAction,
    },
    StopTimer,
    Cancel,
    StayAwake,
    Snooze,
    SaveSettings {
        settings: Settings,
    },
    ConnectAgent {
        name: String,
    },
    InstallAgentSkill {
        name: String,
        update: bool,
    },
    OpenAgentSkillFolder,
    AuthorizeAgent {
        id: String,
        decision: String,
    },
    CancelAgent {
        id: String,
    },
    WaitAgent {
        id: String,
    },
    FinishAgent {
        id: String,
    },
    RevokeAgent {
        id: String,
    },
    AgentPermission {
        id: String,
        action: Option<PowerAction>,
    },
    AgentEnabled,
    AgentKeepAlive,
    AgentLease {
        seconds: u64,
    },
    AgentDefault {
        action: Option<PowerAction>,
    },
    Refresh,
    PreviewCountdown,
    OpenDialog {
        view: DialogView,
    },
    /// Settings on a given page, such as "agents".
    OpenPage {
        page: String,
    },
    /// The tray panel or flyout, anchored to the icon's screen rectangle.
    OpenPanel {
        anchor: serde_json::Value,
    },
    /// One setting by its saved key, such as "snoozeMinutes" or "agents.askBeforeNew".
    Set {
        key: String,
        value: serde_json::Value,
    },
    /// Shows the existing tray submenu ("countdown", "quick" or "support") at the icon.
    ShowMenu {
        name: String,
    },
    ConnectPreview {
        agent: String,
        remove: bool,
    },
    ConnectApply {
        agent: String,
        remove: bool,
        token: String,
    },
    /// A ready-to-paste MCP configuration for another client.
    CopyConfig,
    ExportDiagnostics {
        path: std::path::PathBuf,
    },
    /// Every setting back to its default. Agent credentials are kept so connected tools
    /// keep working; agent trust is reset.
    Reset,
    Quit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogView {
    Panel,
    Settings,
    Help,
    Agents,
    About,
    AwakeDuration,
    AwakeTime,
    TimerDuration,
    TimerTime,
}
pub enum Request {
    Operation(Operation, Sender<Result<Snapshot, String>>),
    Mcp(
        crate::mcp::tools::Call,
        Sender<Result<serde_json::Value, String>>,
    ),
    Lifecycle,
    WarningFailed(String),
    /// The local MCP bridge could not start. The rest of Doze keeps working.
    AgentsUnavailable(String),
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
    /// The Settings page to open with the view, if any.
    pub page: Option<String>,
    /// Where the panel opens: the tray icon's rectangle in physical screen pixels.
    pub panel_anchor: Option<serde_json::Value>,
    /// Data returned by the last operation, such as a config diff. Cleared after the reply.
    pub result: Option<serde_json::Value>,
    /// What the power manager holds right now.
    pub assertions: Vec<String>,
}

impl Snapshot {
    pub fn new(settings_path: std::path::PathBuf, settings: Settings) -> Self {
        Self {
            settings_path,
            engine: Engine::default(),
            selected_action: settings.default_action,
            settings,
            actions: Vec::new(),
            audio_supported: true,
            startup_supported: true,
            error: None,
            view: DialogView::Settings,
            page: None,
            panel_anchor: None,
            result: None,
            assertions: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Preference {
    AllowDisplaySleep,
    Notifications,
    LaunchAtStartup,
    StartMinimized,
    Logging,
}

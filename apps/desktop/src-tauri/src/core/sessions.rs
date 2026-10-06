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

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// What a plain click on the menu bar or tray icon opens. The other button (right-click, or
/// ⌥-click on macOS) always opens the other one.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum IconClick {
    #[default]
    Panel,
    Menu,
}

/// The single settings store for both apps. Field names are the saved JSON keys (camelCase);
/// keys older than the redesigned Settings keep their original names:
/// open at login = `launchAtStartup`, keep the display on = `!allowDisplaySleep`,
/// warning length = `countdownSeconds`, playback inactivity = `idleSeconds`,
/// When agents finish = `agents.defaultCompletion`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub agents: crate::mcp::auth::AgentSettings,
    pub theme: Theme,
    pub launch_at_startup: bool,
    pub start_minimized: bool,
    /// Countdown notifications.
    pub notifications: bool,
    pub default_action: PowerAction,
    pub silence_seconds: u64,
    pub idle_seconds: u64,
    pub countdown_seconds: u64,
    pub playback_action: PowerAction,
    pub logging: bool,
    pub allow_display_sleep: bool,
    pub default_awake_minutes: u64,
    pub default_timer_minutes: u64,
    /// macOS shows the nearest countdown, timer or awake deadline beside the menu bar icon.
    pub menu_bar_time: bool,
    /// macOS shows how many agents are working when only agents hold the Mac awake.
    pub menu_bar_agent_count: bool,
    pub icon_click_opens: IconClick,
    /// Stay awake with the lid closed, while Doze keeps the computer awake. Offered on Windows
    /// laptops; macOS only honours it with root privileges, which Doze does not use.
    pub lid_closed_keep_awake: bool,
    /// On battery below this level every agent and manual keep-awake is released. 0 is off.
    pub battery_floor_percent: u8,
    pub remember_last_custom_duration: bool,
    pub last_custom_awake_minutes: Option<u64>,
    pub snooze_minutes: u64,
    /// What Stay Awake in the final warning does: minutes, or until stopped when empty.
    pub stay_awake_minutes: Option<u64>,
    pub warning_sound: bool,
    pub warning_all_displays: bool,
    pub notify_agent_approval: bool,
    pub notify_agents_finished: bool,
    pub notify_agent_stalled: bool,
    pub notify_keep_awake_ended: bool,
    /// MCP tool calls from agents. Hooks and `doze run` use the same private bridge and keep
    /// working when this is off.
    pub mcp_server_enabled: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            agents: crate::mcp::auth::AgentSettings::default(),
            theme: Theme::System,
            launch_at_startup: true,
            start_minimized: true,
            notifications: true,
            default_action: PowerAction::Sleep,
            silence_seconds: 60,
            idle_seconds: 600,
            countdown_seconds: 300,
            playback_action: PowerAction::Sleep,
            logging: false,
            allow_display_sleep: false,
            default_awake_minutes: 30,
            default_timer_minutes: 30,
            menu_bar_time: true,
            menu_bar_agent_count: true,
            icon_click_opens: IconClick::Panel,
            lid_closed_keep_awake: true,
            battery_floor_percent: 15,
            remember_last_custom_duration: true,
            last_custom_awake_minutes: None,
            snooze_minutes: 15,
            stay_awake_minutes: None,
            warning_sound: true,
            warning_all_displays: false,
            notify_agent_approval: true,
            notify_agents_finished: true,
            notify_agent_stalled: true,
            notify_keep_awake_ended: false,
            mcp_server_enabled: true,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        self.agents.validate()?;
        if !(1..=10080).contains(&self.default_awake_minutes)
            || !(1..=10080).contains(&self.default_timer_minutes)
            || self
                .last_custom_awake_minutes
                .is_some_and(|m| !(1..=10080).contains(&m))
            || self
                .stay_awake_minutes
                .is_some_and(|m| !(1..=1440).contains(&m))
        {
            return Err("Default session durations must be between 1 and 10080 minutes.".into());
        }
        if !(10..=3600).contains(&self.silence_seconds)
            || !(30..=7200).contains(&self.idle_seconds)
            || !(15..=1800).contains(&self.countdown_seconds)
        {
            return Err("Use silence 10–3600s, idle 30–7200s, and countdown 15–1800s.".into());
        }
        if !(1..=120).contains(&self.snooze_minutes) {
            return Err("Snooze must be 1–120 minutes.".into());
        }
        if self.battery_floor_percent > 90 {
            return Err("The battery floor must be 90% or lower.".into());
        }
        Ok(())
    }
    /// The final warning for agents is never shorter than five minutes.
    pub fn agent_warning_seconds(&self) -> u64 {
        self.countdown_seconds.max(300)
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

/// Engine changes the user may be notified about, each behind its own setting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    ApprovalNeeded {
        agent: String,
        project: Option<String>,
    },
    AgentsFinished {
        action: PowerAction,
    },
    AgentStalled {
        agent: String,
    },
    KeepAwakeEnded,
    BatteryGuard {
        percent: u8,
    },
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
    pub agents: crate::mcp::sessions::Sessions,
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
    /// The latest battery reading: percent and whether the computer runs on battery.
    pub battery: Option<(u8, bool)>,
    /// On battery below the floor: agents and manual keep-awake may not hold the computer.
    pub battery_low: bool,
    /// Things worth a notification, drained by the runtime after each step.
    #[serde(skip)]
    pub events: Vec<Event>,
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
            agents: crate::mcp::sessions::Sessions::default(),
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
            battery: None,
            battery_low: false,
            events: Vec::new(),
            silence_since: None,
            audio_streak: 0,
            last_audio_at: None,
        }
    }
}

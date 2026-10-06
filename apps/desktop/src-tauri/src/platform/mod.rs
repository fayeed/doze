use crate::core::sessions::PowerAction;

pub trait PowerManager {
    fn supported_actions(&self) -> Vec<PowerAction>;
    fn set_awake(&mut self, active: bool, allow_display_sleep: bool) -> Result<(), String>;
    fn execute(&mut self, action: PowerAction) -> Result<(), String>;
    /// A note on how the last successful action went, such as Windows using Away Mode
    /// instead of sleep. The engine shows it as the last event.
    fn take_notice(&mut self) -> Option<String> {
        None
    }
    /// The power assertions or requests Doze holds right now, for Settings › Advanced.
    fn describe(&self) -> Vec<String> {
        Vec::new()
    }
}
/// Executable names of running processes, for tools detected by process.
#[cfg(target_os = "macos")]
pub use macos::battery::processes;
/// Battery level in percent and whether the computer runs on battery, if it has one.
#[cfg(target_os = "macos")]
pub use macos::battery::read as battery;
#[cfg(windows)]
pub use windows::battery::processes;
#[cfg(windows)]
pub use windows::battery::read as battery;
pub mod countdown;
#[cfg(windows)]
pub use windows::dialogs;
mod native_ui;
#[cfg(target_os = "macos")]
pub mod dialogs {
    pub fn show(
        snapshot: crate::state::Snapshot,
        requests: std::sync::mpsc::Sender<crate::state::Request>,
    ) -> Result<(), String> {
        super::native_ui::show(snapshot, requests)
    }
}
pub trait AudioMonitor {
    fn sample(&mut self) -> Result<bool, String>;
}
pub struct IdleObservation {
    pub seconds: u64,
    pub activity_marker: Option<u64>,
}
pub trait IdleMonitor {
    fn observe(&self) -> Result<IdleObservation, String>;
}
pub trait NotificationManager {
    fn countdown(&self, action: PowerAction, seconds: u64) -> Result<(), String>;
}
pub fn audio_supported() -> bool {
    true
}
pub fn startup_supported() -> bool {
    true
}

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;
#[cfg(target_os = "macos")]
pub use macos::{audio::NativeAudio, idle::NativeIdle, lifecycle, power::NativePower, startup};
#[cfg(windows)]
pub use windows::{audio::NativeAudio, idle::NativeIdle, lifecycle, power::NativePower, startup};
#[cfg(not(any(windows, target_os = "macos")))]
compile_error!("Doze currently targets Windows and macOS.");

pub use native_ui::publish;

pub struct NativeNotifications(pub tauri::AppHandle);
impl NativeNotifications {
    pub fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        use tauri_plugin_notification::NotificationExt;
        self.0
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| e.to_string())
    }
}
impl NotificationManager for NativeNotifications {
    fn countdown(&self, action: PowerAction, seconds: u64) -> Result<(), String> {
        use tauri_plugin_notification::NotificationExt;
        self.0
            .notification()
            .builder()
            .title("Doze · Power countdown")
            .body(format!(
                "{} in {}m {}s. Open Doze in the {} to cancel or snooze 15 minutes.",
                action.label(),
                seconds / 60,
                seconds % 60,
                if cfg!(target_os = "macos") {
                    "menu bar"
                } else {
                    "tray"
                }
            ))
            .show()
            .map_err(|e| e.to_string())
    }
}

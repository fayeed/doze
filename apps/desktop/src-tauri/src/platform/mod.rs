use crate::core::sessions::PowerAction;

pub trait PowerManager {
    fn supported_actions(&self) -> Vec<PowerAction>;
    fn set_awake(&mut self, active: bool, allow_display_sleep: bool) -> Result<(), String>;
    fn execute(&mut self, action: PowerAction) -> Result<(), String>;
}
#[cfg(target_os = "macos")]
pub use macos::countdown;
#[cfg(windows)]
pub use windows::countdown;
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
    cfg!(windows)
}
pub fn startup_supported() -> bool {
    cfg!(windows)
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

pub struct NativeNotifications(pub tauri::AppHandle);
impl NotificationManager for NativeNotifications {
    fn countdown(&self, action: PowerAction, seconds: u64) -> Result<(), String> {
        use tauri_plugin_notification::NotificationExt;
        self.0
            .notification()
            .builder()
            .title("Doze · Power countdown")
            .body(format!(
                "{} in {}m {}s. Open Doze in the tray to cancel or snooze 15 minutes.",
                action.label(),
                seconds / 60,
                seconds % 60
            ))
            .show()
            .map_err(|e| e.to_string())
    }
}

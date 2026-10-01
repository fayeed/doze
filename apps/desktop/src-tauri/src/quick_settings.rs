//! Native shortcuts to saved preferences, separate from current session controls.
use crate::{
    core::sessions::Settings,
    menu_icons::{self, Glyph},
    state::{Operation, Preference},
};
use tauri::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};

const PREFERENCES: [(Preference, &str); 5] = [
    (
        Preference::AllowDisplaySleep,
        "Allow display sleep during awake sessions",
    ),
    (Preference::Notifications, "Show countdown notifications"),
    (Preference::LaunchAtStartup, "Launch when I sign in"),
    (
        Preference::StartMinimized,
        if cfg!(target_os = "macos") {
            "Start in the menu bar"
        } else {
            "Start in the tray"
        },
    ),
    (Preference::Logging, "Write local diagnostic logs"),
];
type DurationMenu = (Submenu<tauri::Wry>, Vec<(u64, CheckMenuItem<tauri::Wry>)>);
pub(crate) struct QuickSettings {
    pub menu: Submenu<tauri::Wry>,
    preferences: Vec<CheckMenuItem<tauri::Wry>>,
    awake_menu: Submenu<tauri::Wry>,
    timer_menu: Submenu<tauri::Wry>,
    awake: Vec<(u64, CheckMenuItem<tauri::Wry>)>,
    timer: Vec<(u64, CheckMenuItem<tauri::Wry>)>,
}
fn durations(app: &tauri::App, prefix: &str, title: &str) -> tauri::Result<DurationMenu> {
    let menu = Submenu::new(app, title, true)?;
    let mut checks = Vec::new();
    for minutes in [15, 30, 60, 120] {
        let item = CheckMenuItem::with_id(
            app,
            format!("{prefix}:{minutes}"),
            format!("{minutes} minutes"),
            true,
            minutes == 30,
            None::<&str>,
        )?;
        menu.append(&item)?;
        checks.push((minutes, item));
    }
    Ok((menu, checks))
}
impl QuickSettings {
    pub fn new(app: &tauri::App) -> tauri::Result<Self> {
        let menu = menu_icons::submenu(app, "quick_menu", "Quick Settings", Glyph::Quick)?;
        menu.append(&MenuItem::with_id(
            app,
            "quick_info",
            "Saved preferences (not session status)",
            false,
            None::<&str>,
        )?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        let mut preferences = Vec::new();
        for (index, (preference, label)) in PREFERENCES.iter().enumerate() {
            let enabled = !matches!(preference, Preference::LaunchAtStartup)
                || crate::platform::startup_supported();
            let item = CheckMenuItem::with_id(
                app,
                format!("preference:{index}"),
                *label,
                enabled,
                false,
                None::<&str>,
            )?;
            menu.append(&item)?;
            preferences.push(item);
        }
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        let (awake_menu, awake) = durations(app, "default_awake", "Default awake duration")?;
        let (timer_menu, timer) = durations(app, "default_timer", "Default timer duration")?;
        menu.append(&awake_menu)?;
        menu.append(&timer_menu)?;
        menu.append(&MenuItem::with_id(
            app,
            "settings",
            "All Settings…",
            true,
            None::<&str>,
        )?)?;
        Ok(Self {
            menu,
            preferences,
            awake_menu,
            timer_menu,
            awake,
            timer,
        })
    }
    pub fn update(&self, settings: &Settings) {
        for (item, checked) in self.preferences.iter().zip([
            settings.allow_display_sleep,
            settings.notifications,
            settings.launch_at_startup,
            settings.start_minimized,
            settings.logging,
        ]) {
            let _ = item.set_checked(checked);
        }
        let _ = self.awake_menu.set_text(format!(
            "Default awake: {} minutes",
            settings.default_awake_minutes
        ));
        let _ = self.timer_menu.set_text(format!(
            "Default timer: {} minutes",
            settings.default_timer_minutes
        ));
        for (minutes, item) in &self.awake {
            let _ = item.set_checked(*minutes == settings.default_awake_minutes);
        }
        for (minutes, item) in &self.timer {
            let _ = item.set_checked(*minutes == settings.default_timer_minutes);
        }
    }
}
pub(crate) fn operation(id: &str) -> Option<Operation> {
    let (prefix, value) = id.split_once(':')?;
    let value = value.parse::<u64>().ok()?;
    match prefix {
        "preference" => {
            PREFERENCES
                .get(value as usize)
                .map(|(preference, _)| Operation::TogglePreference {
                    preference: *preference,
                })
        }
        "default_awake" | "default_timer" => Some(Operation::SetDefaultDuration {
            awake: prefix == "default_awake",
            minutes: value,
        }),
        _ => None,
    }
}

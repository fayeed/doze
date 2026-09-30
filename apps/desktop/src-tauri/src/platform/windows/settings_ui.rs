//! Native tabbed preferences. Every control stays alive so switching tabs preserves edits.
use super::dialog_template::Template;
use crate::{
    core::sessions::{PowerAction, Settings},
    state::Snapshot,
};
use std::os::windows::ffi::OsStrExt;
use windows::{
    core::{w, PCWSTR, PWSTR},
    Win32::{
        Foundation::{BOOL, HWND, LPARAM, WPARAM},
        System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
        UI::{
            Controls::{
                CheckDlgButton, IsDlgButtonChecked, BST_CHECKED, BST_UNCHECKED, TCIF_TEXT, TCITEMW,
                TCM_GETCURSEL, TCM_GETITEMCOUNT, TCM_INSERTITEMW,
            },
            Input::KeyboardAndMouse::EnableWindow,
            Shell::ShellExecuteW,
            WindowsAndMessaging::*,
        },
    },
};

pub(super) const TABS: u16 = 150;
pub(super) const RESET: u16 = 110;
pub(super) const PREVIEW: u16 = 114;
pub(super) const OPEN_DATA: u16 = 115;
const STARTUP: u16 = 101;
const MINIMIZED: u16 = 102;
const NOTIFICATIONS: u16 = 103;
const ACTION: u16 = 104;
const SILENCE: u16 = 105;
const IDLE: u16 = 106;
const COUNTDOWN: u16 = 107;
const PLAYBACK_ACTION: u16 = 108;
const LOGGING: u16 = 109;
const DISPLAY_SLEEP: u16 = 111;
const AWAKE_MINUTES: u16 = 112;
const TIMER_MINUTES: u16 = 113;
const DIAGNOSTICS: u16 = 116;
const PAGES: [&[u16]; 5] = [
    &[STARTUP, MINIMIZED, 200, 201, 202, 203],
    &[
        DISPLAY_SLEEP,
        AWAKE_MINUTES,
        TIMER_MINUTES,
        ACTION,
        204,
        205,
        206,
        207,
        208,
    ],
    &[PLAYBACK_ACTION, SILENCE, IDLE, 209, 210, 211, 212, 213, 214],
    &[NOTIFICATIONS, COUNTDOWN, PREVIEW, 215, 216, 217, 218],
    &[LOGGING, OPEN_DATA, DIAGNOSTICS, 219, 220],
];

pub(super) fn template() -> Vec<u32> {
    let mut form = Template::new("Doze Settings", 400, 278);
    form.tabs(TABS, [12, 12, 376, 230]);
    form.checkbox_at(STARTUP, "Launch Doze when I sign in", [24, 58, 350, 14]);
    form.label_with_id(
        200,
        "Start Doze automatically when you sign in to Windows.",
        24,
        78,
        350,
    );
    form.checkbox_at(MINIMIZED, "Start in the tray", [24, 106, 350, 14]);
    form.label_with_id(
        201,
        "Keep Settings closed at launch. Sessions are started from the tray.",
        24,
        126,
        350,
    );
    form.label_with_id(
        202,
        "Sessions are cleared after restart or suspend/resume.",
        24,
        170,
        350,
    );
    form.label_with_id(
        203,
        "Save applies your changes; Cancel discards unsaved edits.",
        24,
        192,
        350,
    );

    form.checkbox_at(
        DISPLAY_SLEEP,
        "Allow display sleep while keeping the computer awake",
        [24, 58, 350, 14],
    );
    form.label_with_id(
        204,
        "The system stays awake; Windows can turn off the screen.",
        24,
        78,
        350,
    );
    for (id, label_id, text, y) in [
        (
            AWAKE_MINUTES,
            205,
            "Default Keep Awake duration (minutes)",
            114,
        ),
        (TIMER_MINUTES, 206, "Default timer duration (minutes)", 146),
    ] {
        form.label_with_id(label_id, text, 24, y + 3, 216);
        form.edit(id, "", 246, y, 128);
    }
    form.label_with_id(207, "Default timer action", 24, 181, 216);
    form.combo_at(ACTION, [246, 178, 128, 90]);
    form.label_with_id(
        208,
        "Durations apply to new sessions. Display sleep changes take effect now.",
        24,
        216,
        350,
    );

    form.label_with_id(209, "Action after playback stops", 24, 61, 216);
    form.combo_at(PLAYBACK_ACTION, [246, 58, 128, 90]);
    for (id, label_id, text, y) in [
        (SILENCE, 210, "Silence grace (seconds, 10–3600)", 94),
        (IDLE, 211, "Required idle time (seconds, 30–7200)", 126),
    ] {
        form.label_with_id(label_id, text, 24, y + 3, 216);
        form.edit(id, "", 246, y, 128);
    }
    form.label_with_id(
        212,
        "Ongoing playback must be observed first; brief sounds do not arm it.",
        24,
        168,
        350,
    );
    form.label_with_id(
        213,
        "Silence and idle checks must both pass before the final countdown.",
        24,
        190,
        350,
    );
    form.label_with_id(
        214,
        "Resumed audio or user activity cancels the automatic action.",
        24,
        212,
        350,
    );

    form.checkbox_at(
        NOTIFICATIONS,
        "Show Windows countdown notifications",
        [24, 58, 350, 14],
    );
    form.label_with_id(
        215,
        "The native warning window still provides Cancel and Snooze.",
        24,
        78,
        350,
    );
    form.label_with_id(216, "Final countdown (seconds, 15–1800)", 24, 117, 216);
    form.edit(COUNTDOWN, "", 246, 114, 128);
    form.label_with_id(
        217,
        "All timer and playback actions use this final warning.",
        24,
        146,
        350,
    );
    form.button(PREVIEW, "Preview", 24, 182, false);
    form.label_with_id(
        218,
        "Try the warning without scheduling a power action.",
        96,
        185,
        278,
    );

    form.checkbox_at(LOGGING, "Write local diagnostic logs", [24, 58, 350, 14]);
    form.label_with_id(
        219,
        "Record errors locally for troubleshooting. Logging is off by default.",
        24,
        78,
        350,
    );
    form.button(OPEN_DATA, "Open data", 24, 112, false);
    form.label_with_id(
        220,
        "View local settings and diagnostic logs.",
        96,
        115,
        278,
    );
    form.read_only_text(DIAGNOSTICS, [24, 150, 350, 78]);
    form.button(RESET, "Reset all", 12, 252, false);
    form.button(1, "Save", 252, 252, true);
    form.button(2, "Cancel", 324, 252, false);
    form.finish()
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

pub(super) unsafe fn initialize(hwnd: HWND, snapshot: &Snapshot) {
    if SendDlgItemMessageW(hwnd, TABS as i32, TCM_GETITEMCOUNT, WPARAM(0), LPARAM(0)).0 == 0 {
        for (index, name) in [
            "General",
            "Session Defaults",
            "After Playback",
            "Notifications",
            "Advanced",
        ]
        .iter()
        .enumerate()
        {
            let mut text = wide(name);
            let tab = TCITEMW {
                mask: TCIF_TEXT,
                pszText: PWSTR(text.as_mut_ptr()),
                ..Default::default()
            };
            SendDlgItemMessageW(
                hwnd,
                TABS as i32,
                TCM_INSERTITEMW,
                WPARAM(index),
                LPARAM((&tab as *const TCITEMW) as isize),
            );
        }
    }
    let settings = &snapshot.settings;
    for (id, checked) in [
        (STARTUP, settings.launch_at_startup),
        (MINIMIZED, settings.start_minimized),
        (NOTIFICATIONS, settings.notifications),
        (LOGGING, settings.logging),
        (DISPLAY_SLEEP, settings.allow_display_sleep),
    ] {
        let _ = CheckDlgButton(
            hwnd,
            id as i32,
            if checked { BST_CHECKED } else { BST_UNCHECKED },
        );
    }
    if let Ok(control) = GetDlgItem(hwnd, STARTUP as i32) {
        let _ = EnableWindow(control, snapshot.startup_supported);
    }
    for (id, selected) in [
        (ACTION, settings.default_action),
        (PLAYBACK_ACTION, settings.playback_action),
    ] {
        SendDlgItemMessageW(hwnd, id as i32, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        for (index, action) in snapshot.actions.iter().enumerate() {
            let text = wide(action.label());
            SendDlgItemMessageW(
                hwnd,
                id as i32,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(text.as_ptr() as isize),
            );
            if *action == selected {
                SendDlgItemMessageW(hwnd, id as i32, CB_SETCURSEL, WPARAM(index), LPARAM(0));
            }
        }
    }
    for (id, value) in [
        (SILENCE, settings.silence_seconds),
        (IDLE, settings.idle_seconds),
        (COUNTDOWN, settings.countdown_seconds),
        (AWAKE_MINUTES, settings.default_awake_minutes),
        (TIMER_MINUTES, settings.default_timer_minutes),
    ] {
        let text = wide(&value.to_string());
        let _ = SetDlgItemTextW(hwnd, id as i32, PCWSTR(text.as_ptr()));
    }
    let actions = snapshot
        .actions
        .iter()
        .map(|action| action.label())
        .collect::<Vec<_>>()
        .join(", ");
    let details = format!("Privacy: no accounts, cloud connection, ads or telemetry.\r\nSettings file: {}\r\nAvailable actions: {actions}\r\nAudio monitoring: {}\r\nCurrent status: {}", snapshot.settings_path.display(), if snapshot.audio_supported { "available" } else { "unavailable" }, crate::tray::status_text(snapshot));
    let text = wide(&details);
    let _ = SetDlgItemTextW(hwnd, DIAGNOSTICS as i32, PCWSTR(text.as_ptr()));
    select_page(hwnd);
}

pub(super) unsafe fn select_page(hwnd: HWND) {
    let selected = SendDlgItemMessageW(hwnd, TABS as i32, TCM_GETCURSEL, WPARAM(0), LPARAM(0))
        .0
        .max(0) as usize;
    for (page, controls) in PAGES.iter().enumerate() {
        for id in *controls {
            if let Ok(control) = GetDlgItem(hwnd, *id as i32) {
                let _ = ShowWindow(control, if page == selected { SW_SHOW } else { SW_HIDE });
            }
        }
    }
}
unsafe fn number(hwnd: HWND, id: u16) -> Result<u64, String> {
    let mut valid = BOOL(0);
    let value = GetDlgItemInt(hwnd, id as i32, Some(&mut valid), false);
    if valid.as_bool() {
        Ok(u64::from(value))
    } else {
        Err("Enter a whole number in every duration field.".into())
    }
}
unsafe fn action(hwnd: HWND, id: u16, actions: &[PowerAction]) -> Result<PowerAction, String> {
    let index = SendDlgItemMessageW(hwnd, id as i32, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    actions
        .get(index as usize)
        .copied()
        .ok_or("Choose a supported power action.".into())
}
pub(super) unsafe fn collect(hwnd: HWND, snapshot: &Snapshot) -> Result<Settings, String> {
    let checked = |id| IsDlgButtonChecked(hwnd, id) == BST_CHECKED.0;
    let settings = Settings {
        launch_at_startup: checked(STARTUP as i32),
        start_minimized: checked(MINIMIZED as i32),
        notifications: checked(NOTIFICATIONS as i32),
        logging: checked(LOGGING as i32),
        allow_display_sleep: checked(DISPLAY_SLEEP as i32),
        default_awake_minutes: number(hwnd, AWAKE_MINUTES)?,
        default_timer_minutes: number(hwnd, TIMER_MINUTES)?,
        default_action: action(hwnd, ACTION, &snapshot.actions)?,
        playback_action: action(hwnd, PLAYBACK_ACTION, &snapshot.actions)?,
        silence_seconds: number(hwnd, SILENCE)?,
        idle_seconds: number(hwnd, IDLE)?,
        countdown_seconds: number(hwnd, COUNTDOWN)?,
    };
    settings.validate()?;
    Ok(settings)
}
pub(super) unsafe fn reset_draft(hwnd: HWND, snapshot: &Snapshot) {
    let mut draft = snapshot.clone();
    draft.settings = Settings::default();
    if !draft.actions.contains(&draft.settings.default_action) {
        if let Some(action) = draft.actions.first().copied() {
            draft.settings.default_action = action;
            draft.settings.playback_action = action;
        }
    }
    initialize(hwnd, &draft);
}
pub(super) fn open_data(hwnd: HWND, snapshot: &Snapshot) -> Result<(), String> {
    let folder = snapshot
        .settings_path
        .parent()
        .ok_or("Settings folder is unavailable.")?;
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let path: Vec<u16> = folder.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|error| error.to_string())?;
        let value = ShellExecuteW(
            hwnd,
            w!("open"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        CoUninitialize();
        value
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err("Windows could not open the data folder.".into())
    }
}

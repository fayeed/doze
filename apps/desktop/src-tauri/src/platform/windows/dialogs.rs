use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
// Modal native dialogs with standard buttons, checkboxes, edits and date/time pickers.
use super::dialog_template::Template;
use crate::{
    core::sessions::{PowerAction, Settings},
    state::{DialogView, Operation, Request, Snapshot},
};
use std::{
    sync::{
        atomic::{AtomicIsize, Ordering},
        mpsc::{self, Sender},
    },
    time::Duration,
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{BOOL, HINSTANCE, HWND, LPARAM, SYSTEMTIME, WPARAM},
        System::{
            SystemInformation::GetSystemTimeAsFileTime,
            Time::{SystemTimeToFileTime, TzSpecificLocalTimeToSystemTimeEx},
        },
        UI::{
            Controls::{
                CheckDlgButton, InitCommonControlsEx, IsDlgButtonChecked, BST_CHECKED,
                BST_UNCHECKED, DTM_GETSYSTEMTIME, ICC_DATE_CLASSES, INITCOMMONCONTROLSEX,
            },
            WindowsAndMessaging::*,
        },
    },
};

const DIALOG_USER_OFFSET: WINDOW_LONG_PTR_INDEX =
    WINDOW_LONG_PTR_INDEX((2 * std::mem::size_of::<isize>()) as i32);
const STARTUP: u16 = 101;
const MINIMIZED: u16 = 102;
const NOTIFICATIONS: u16 = 103;
const ACTION: u16 = 104;
const SILENCE: u16 = 105;
const IDLE: u16 = 106;
const COUNTDOWN: u16 = 107;
const PLAYBACK_ACTION: u16 = 108;
const LOGGING: u16 = 109;
const RESET: u16 = 110;
const MINUTES: u16 = 120;
const DATE: u16 = 121;
const TIME: u16 = 122;
const HELP_TEXT: u16 = 123;
// Only the main thread accesses a dialog. Repeated tray clicks focus the existing dialog.
static ACTIVE_DIALOG: AtomicIsize = AtomicIsize::new(0);

struct Dialog {
    snapshot: Snapshot,
    sender: Sender<Request>,
}

pub fn show(snapshot: Snapshot, sender: Sender<Request>) -> Result<(), String> {
    let active = ACTIVE_DIALOG.load(Ordering::Relaxed);
    if active != 0 {
        unsafe {
            let _ = SetForegroundWindow(HWND(active as *mut _));
        }
        return Ok(());
    }
    let controls = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_DATE_CLASSES,
    };
    if !unsafe { InitCommonControlsEx(&controls) }.as_bool() {
        return Err("Could not initialize Windows date/time controls.".into());
    }
    let template = template(snapshot.view);
    let mut dialog = Box::new(Dialog { snapshot, sender });
    // Both allocations stay alive for the entire modal message loop.
    let result = unsafe {
        DialogBoxIndirectParamW(
            HINSTANCE::default(),
            template.as_ptr().cast(),
            HWND::default(),
            Some(dialog_proc),
            LPARAM((&mut *dialog as *mut Dialog) as isize),
        )
    };
    ACTIVE_DIALOG.store(0, Ordering::Relaxed);
    if result == -1 {
        Err(windows::core::Error::from_win32().to_string())
    } else {
        Ok(())
    }
}

fn template(view: DialogView) -> Vec<u32> {
    if matches!(view, DialogView::Help) {
        let mut form = Template::new("Doze · What do these options mean?", 360, 240);
        form.read_only_text(HELP_TEXT, [12, 12, 336, 192]);
        form.button(2, "Close", 286, 216, true);
        form.finish()
    } else if matches!(view, DialogView::Settings) {
        let mut form = Template::new("Doze Settings", 278, 282);
        form.checkbox(STARTUP, "Launch Doze when I sign in", 12);
        form.checkbox(MINIMIZED, "Start in the tray", 31);
        form.checkbox(NOTIFICATIONS, "Show countdown notifications", 50);
        form.label("Default timer action", 12, 76, 124);
        form.combo(ACTION, 73);
        form.label("After Playback", 12, 103, 240);
        form.label("Action", 12, 126, 124);
        form.combo(PLAYBACK_ACTION, 123);
        for (id, text, y) in [
            (SILENCE, "Silence required (seconds)", 149),
            (IDLE, "Idle required (seconds)", 171),
            (COUNTDOWN, "Countdown (seconds)", 193),
        ] {
            form.label(text, 12, y, 124);
            form.edit(id, "", 142, y - 2, 122);
        }
        form.checkbox(LOGGING, "Write local diagnostic logs", 218);
        form.button(RESET, "Reset", 12, 252, false);
        form.button(1, "Save", 136, 252, true);
        form.button(2, "Cancel", 204, 252, false);
        form.finish()
    } else {
        let awake = matches!(view, DialogView::AwakeDuration | DialogView::AwakeTime);
        let mut form = Template::new(if awake { "Keep awake" } else { "Sleep timer" }, 278, 108);
        if matches!(view, DialogView::AwakeTime | DialogView::TimerTime) {
            form.label(
                "Choose a future date and local time (within 7 days)",
                12,
                12,
                252,
            );
            form.date_time(DATE, 12, 36, 130, false);
            form.date_time(TIME, 150, 36, 114, true);
        } else {
            form.label("Duration in minutes (1–10080)", 12, 12, 252);
            form.edit(MINUTES, "30", 12, 36, 252);
        }
        form.button(1, "Start", 136, 78, true);
        form.button(2, "Cancel", 204, 78, false);
        form.finish()
    }
}

unsafe extern "system" fn dialog_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    if message == WM_INITDIALOG {
        SetWindowLongPtrW(hwnd, DIALOG_USER_OFFSET, lparam.0);
        ACTIVE_DIALOG.store(hwnd.0 as isize, Ordering::Relaxed);
        let dialog = &*(lparam.0 as *const Dialog);
        initialize(hwnd, &dialog.snapshot);
        if matches!(dialog.snapshot.view, DialogView::Help) {
            if let Ok(close) = GetDlgItem(hwnd, 2) {
                let _ = SetFocus(close);
            }
            return 0;
        }
        return 1;
    }
    if message == WM_CLOSE {
        let _ = EndDialog(hwnd, 0);
        return 1;
    }
    if message != WM_COMMAND {
        return 0;
    }
    let id = (wparam.0 & 0xffff) as u16;
    if id == 2 {
        let _ = EndDialog(hwnd, 0);
        return 1;
    }
    if id != 1 && id != RESET {
        return 0;
    }
    let context = GetWindowLongPtrW(hwnd, DIALOG_USER_OFFSET) as *mut Dialog;
    if context.is_null() {
        return 0;
    }
    let dialog = &mut *context;
    if matches!(dialog.snapshot.view, DialogView::Help) {
        let _ = EndDialog(hwnd, 0);
        return 1;
    }
    let operation = if id == RESET {
        Ok(Operation::ResetSettings)
    } else {
        collect(hwnd, &dialog.snapshot)
    };
    match operation.and_then(|operation| submit(&dialog.sender, operation)) {
        Ok(snapshot) => {
            if id == RESET {
                dialog.snapshot = snapshot;
                initialize(hwnd, &dialog.snapshot);
            } else {
                let _ = EndDialog(hwnd, 1);
            }
        }
        Err(error) => {
            let message = wide(&error);
            MessageBoxW(
                hwnd,
                PCWSTR(message.as_ptr()),
                w!("Doze"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    1
}

fn submit(sender: &Sender<Request>, operation: Operation) -> Result<Snapshot, String> {
    let (reply, receiver) = mpsc::channel();
    sender
        .send(Request::Operation(operation, reply))
        .map_err(|_| "Doze engine stopped.".to_string())?;
    receiver
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| "Doze engine did not respond.".to_string())?
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

unsafe fn initialize(hwnd: HWND, snapshot: &Snapshot) {
    if matches!(snapshot.view, DialogView::Help) {
        let text = wide(&super::help::text(snapshot).replace('\n', "\r\n"));
        let _ = SetDlgItemTextW(hwnd, HELP_TEXT as i32, PCWSTR(text.as_ptr()));
        return;
    }
    if !matches!(snapshot.view, DialogView::Settings) {
        return;
    }
    let settings = &snapshot.settings;
    for (id, checked) in [
        (STARTUP, settings.launch_at_startup),
        (MINIMIZED, settings.start_minimized),
        (NOTIFICATIONS, settings.notifications),
        (LOGGING, settings.logging),
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
    ] {
        let text = wide(&value.to_string());
        let _ = SetDlgItemTextW(hwnd, id as i32, PCWSTR(text.as_ptr()));
    }
}
unsafe fn number(hwnd: HWND, id: u16) -> Result<u64, String> {
    let mut valid = BOOL(0);
    let value = GetDlgItemInt(hwnd, id as i32, Some(&mut valid), false);
    if !valid.as_bool() {
        Err("Enter a whole number in each duration field.".into())
    } else {
        Ok(u64::from(value))
    }
}
unsafe fn selected(hwnd: HWND, id: u16, actions: &[PowerAction]) -> Result<PowerAction, String> {
    let index = SendDlgItemMessageW(hwnd, id as i32, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    actions
        .get(index as usize)
        .copied()
        .ok_or("Choose a supported action.".into())
}
unsafe fn collect(hwnd: HWND, snapshot: &Snapshot) -> Result<Operation, String> {
    if matches!(snapshot.view, DialogView::Settings) {
        let settings = Settings {
            launch_at_startup: IsDlgButtonChecked(hwnd, STARTUP as i32) == BST_CHECKED.0,
            start_minimized: IsDlgButtonChecked(hwnd, MINIMIZED as i32) == BST_CHECKED.0,
            notifications: IsDlgButtonChecked(hwnd, NOTIFICATIONS as i32) == BST_CHECKED.0,
            default_action: selected(hwnd, ACTION, &snapshot.actions)?,
            playback_action: selected(hwnd, PLAYBACK_ACTION, &snapshot.actions)?,
            silence_seconds: number(hwnd, SILENCE)?,
            idle_seconds: number(hwnd, IDLE)?,
            countdown_seconds: number(hwnd, COUNTDOWN)?,
            logging: IsDlgButtonChecked(hwnd, LOGGING as i32) == BST_CHECKED.0,
        };
        settings.validate()?;
        return Ok(Operation::SaveSettings { settings });
    }
    let seconds = if matches!(snapshot.view, DialogView::AwakeTime | DialogView::TimerTime) {
        future_seconds(hwnd)?
    } else {
        let minutes = number(hwnd, MINUTES)?;
        if !(1..=10080).contains(&minutes) {
            return Err("Choose between 1 and 10080 minutes.".into());
        }
        minutes * 60
    };
    if matches!(
        snapshot.view,
        DialogView::AwakeDuration | DialogView::AwakeTime
    ) {
        Ok(Operation::KeepAwake {
            seconds: Some(seconds),
        })
    } else {
        Ok(Operation::Schedule {
            seconds,
            action: snapshot.selected_action,
        })
    }
}
unsafe fn future_seconds(hwnd: HWND) -> Result<u64, String> {
    let mut date = SYSTEMTIME::default();
    let mut time = SYSTEMTIME::default();
    for (id, value) in [(DATE, &mut date), (TIME, &mut time)] {
        if SendDlgItemMessageW(
            hwnd,
            id as i32,
            DTM_GETSYSTEMTIME,
            WPARAM(0),
            LPARAM((value as *mut SYSTEMTIME) as isize),
        )
        .0 != 0
        {
            return Err("Choose a valid date and time.".into());
        }
    }
    date.wHour = time.wHour;
    date.wMinute = time.wMinute;
    date.wSecond = 0;
    date.wMilliseconds = 0;
    let mut utc = SYSTEMTIME::default();
    TzSpecificLocalTimeToSystemTimeEx(None, &date, &mut utc).map_err(|e| e.to_string())?;
    let mut deadline = Default::default();
    SystemTimeToFileTime(&utc, &mut deadline).map_err(|e| e.to_string())?;
    let now = GetSystemTimeAsFileTime();
    let ticks = |value: windows::Win32::Foundation::FILETIME| {
        (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
    };
    let seconds = ticks(deadline).saturating_sub(ticks(now)) / 10_000_000;
    if !(1..=604800).contains(&seconds) {
        Err("Choose a future date and time within 7 days.".into())
    } else {
        Ok(seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::sessions::Engine;

    unsafe extern "system" fn test_proc(_: HWND, _: u32, _: WPARAM, _: LPARAM) -> isize {
        0
    }

    #[test]
    fn native_forms_create_and_validate_without_power_actions() {
        let controls = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_DATE_CLASSES,
        };
        assert!(unsafe { InitCommonControlsEx(&controls) }.as_bool());
        let mut snapshot = Snapshot {
            engine: Engine::default(),
            settings: Settings::default(),
            actions: vec![PowerAction::Sleep, PowerAction::Lock],
            audio_supported: true,
            startup_supported: true,
            error: None,
            selected_action: PowerAction::Lock,
            view: DialogView::Settings,
        };
        for view in [
            DialogView::Settings,
            DialogView::Help,
            DialogView::AwakeDuration,
            DialogView::TimerDuration,
            DialogView::AwakeTime,
            DialogView::TimerTime,
        ] {
            snapshot.view = view;
            let template = template(view);
            // These test windows remain hidden. They never submit operations to the worker.
            let hwnd = unsafe {
                CreateDialogIndirectParamW(
                    HINSTANCE::default(),
                    template.as_ptr().cast(),
                    HWND::default(),
                    Some(test_proc),
                    LPARAM(0),
                )
            }
            .expect("Windows must accept the dialog template");
            unsafe {
                initialize(hwnd, &snapshot);
                match view {
                    DialogView::Help => {
                        assert!(GetDlgItem(hwnd, HELP_TEXT as i32).is_ok());
                        assert!(GetDlgItem(hwnd, MINUTES as i32).is_err());
                    }
                    DialogView::Settings => {
                        let Operation::SaveSettings { settings } =
                            collect(hwnd, &snapshot).unwrap()
                        else {
                            panic!("Expected settings");
                        };
                        assert_eq!(settings, snapshot.settings);
                        SetDlgItemTextW(hwnd, SILENCE as i32, w!("0")).unwrap();
                        assert!(collect(hwnd, &snapshot).is_err());
                    }
                    DialogView::AwakeDuration => assert!(matches!(
                        collect(hwnd, &snapshot).unwrap(),
                        Operation::KeepAwake {
                            seconds: Some(1800)
                        }
                    )),
                    DialogView::TimerDuration => assert!(matches!(
                        collect(hwnd, &snapshot).unwrap(),
                        Operation::Schedule {
                            seconds: 1800,
                            action: PowerAction::Lock
                        }
                    )),
                    DialogView::AwakeTime | DialogView::TimerTime => {
                        assert!(GetDlgItem(hwnd, DATE as i32).is_ok());
                        assert!(GetDlgItem(hwnd, TIME as i32).is_ok());
                        assert!(future_seconds(hwnd).is_err()); // Default is the current minute, already elapsed.
                    }
                }
                DestroyWindow(hwnd).unwrap();
            }
        }
    }
}

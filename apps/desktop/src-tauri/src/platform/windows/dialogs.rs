use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
// Modal native dialogs with standard buttons, checkboxes, edits and date/time pickers.
use super::{
    dialog_template::Template,
    settings_ui::{self, RESET},
};
use crate::state::{DialogView, Operation, Request, Snapshot};
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
                InitCommonControlsEx, DTM_GETSYSTEMTIME, ICC_DATE_CLASSES, ICC_TAB_CLASSES,
                INITCOMMONCONTROLSEX, NMHDR, TCN_SELCHANGE,
            },
            WindowsAndMessaging::*,
        },
    },
};

const DIALOG_USER_OFFSET: WINDOW_LONG_PTR_INDEX =
    WINDOW_LONG_PTR_INDEX((2 * std::mem::size_of::<isize>()) as i32);
const MINUTES: u16 = 120;
const DATE: u16 = 121;
const TIME: u16 = 122;
const HELP_TEXT: u16 = 123;
// Each window owns a native message loop. The tray remains responsive while settings are open.
static ACTIVE_DIALOGS: [AtomicIsize; 4] = [const { AtomicIsize::new(0) }; 4];

fn dialog_slot(view: DialogView) -> usize {
    match view {
        DialogView::Settings => 0,
        DialogView::About => 1,
        DialogView::Help => 2,
        _ => 3,
    }
}

struct Dialog {
    snapshot: Snapshot,
    sender: Sender<Request>,
}

pub fn show(snapshot: Snapshot, sender: Sender<Request>) -> Result<(), String> {
    let slot = dialog_slot(snapshot.view);
    if let Err(active) =
        ACTIVE_DIALOGS[slot].compare_exchange(0, -1, Ordering::Relaxed, Ordering::Relaxed)
    {
        if active > 0 {
            unsafe {
                let _ = SetForegroundWindow(HWND(active as *mut _));
            }
        }
        return Ok(());
    }
    if let Err(error) = std::thread::Builder::new()
        .name("doze-dialog".into())
        .spawn(move || {
            let result = show_modal(snapshot, sender);
            ACTIVE_DIALOGS[slot].store(0, Ordering::Relaxed);
            if let Err(error) = result {
                unsafe {
                    show_error(HWND::default(), &error);
                }
            }
        })
    {
        ACTIVE_DIALOGS[slot].store(0, Ordering::Relaxed);
        return Err(error.to_string());
    }
    Ok(())
}

fn show_modal(snapshot: Snapshot, sender: Sender<Request>) -> Result<(), String> {
    let controls = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_DATE_CLASSES | ICC_TAB_CLASSES,
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
    } else if matches!(view, DialogView::About) {
        let mut form = Template::new("About Doze", 360, 240);
        form.read_only_text(HELP_TEXT, [12, 12, 336, 192]);
        form.button(2, "Close", 286, 216, true);
        form.finish()
    } else if matches!(view, DialogView::Settings) {
        settings_ui::template()
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
        let dialog = &*(lparam.0 as *const Dialog);
        ACTIVE_DIALOGS[dialog_slot(dialog.snapshot.view)].store(hwnd.0 as isize, Ordering::Relaxed);
        initialize(hwnd, &dialog.snapshot);
        if matches!(dialog.snapshot.view, DialogView::Help | DialogView::About) {
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
    if message == WM_NOTIFY && lparam.0 != 0 {
        let header = &*(lparam.0 as *const NMHDR);
        if header.idFrom == settings_ui::TABS as usize && header.code == TCN_SELCHANGE {
            settings_ui::select_page(hwnd);
            return 1;
        }
    }
    if message != WM_COMMAND {
        return 0;
    }
    let id = (wparam.0 & 0xffff) as u16;
    if id == 2 {
        let _ = EndDialog(hwnd, 0);
        return 1;
    }
    let context = GetWindowLongPtrW(hwnd, DIALOG_USER_OFFSET) as *mut Dialog;
    if context.is_null() {
        return 0;
    }
    let dialog = &mut *context;
    if matches!(dialog.snapshot.view, DialogView::Settings)
        && (id == settings_ui::PREVIEW || id == settings_ui::OPEN_DATA)
    {
        let result = if id == settings_ui::PREVIEW {
            submit(&dialog.sender, Operation::PreviewCountdown).map(|_| ())
        } else {
            settings_ui::open_data(hwnd, &dialog.snapshot)
        };
        if let Err(error) = result {
            show_error(hwnd, &error);
        }
        return 1;
    }
    if id != 1 && id != RESET {
        return 0;
    }
    if id == RESET && matches!(dialog.snapshot.view, DialogView::Settings) {
        settings_ui::reset_draft(hwnd, &dialog.snapshot);
        return 1;
    }
    if matches!(dialog.snapshot.view, DialogView::Help | DialogView::About) {
        let _ = EndDialog(hwnd, 0);
        return 1;
    }
    let operation = collect(hwnd, &dialog.snapshot);
    match operation.and_then(|operation| submit(&dialog.sender, operation)) {
        Ok(_) => {
            let _ = EndDialog(hwnd, 1);
        }
        Err(error) => {
            show_error(hwnd, &error);
        }
    }
    1
}

unsafe fn show_error(hwnd: HWND, error: &str) {
    let message = wide(error);
    MessageBoxW(
        hwnd,
        PCWSTR(message.as_ptr()),
        w!("Doze"),
        MB_OK | MB_ICONERROR,
    );
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
    if matches!(snapshot.view, DialogView::Help | DialogView::About) {
        let content = if matches!(snapshot.view, DialogView::About) {
            super::help::about(snapshot)
        } else {
            super::help::text(snapshot)
        };
        let text = wide(&content.replace('\n', "\r\n"));
        let _ = SetDlgItemTextW(hwnd, HELP_TEXT as i32, PCWSTR(text.as_ptr()));
    } else if matches!(snapshot.view, DialogView::Settings) {
        settings_ui::initialize(hwnd, snapshot);
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
unsafe fn collect(hwnd: HWND, snapshot: &Snapshot) -> Result<Operation, String> {
    if matches!(snapshot.view, DialogView::Settings) {
        return Ok(Operation::SaveSettings {
            settings: settings_ui::collect(hwnd, snapshot)?,
        });
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
    use crate::core::sessions::{Engine, PowerAction, Settings};

    unsafe extern "system" fn test_proc(_: HWND, _: u32, _: WPARAM, _: LPARAM) -> isize {
        0
    }

    #[test]
    fn native_forms_create_and_validate_without_power_actions() {
        let controls = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_DATE_CLASSES | ICC_TAB_CLASSES,
        };
        assert!(unsafe { InitCommonControlsEx(&controls) }.as_bool());
        let mut snapshot = Snapshot {
            settings_path: "settings.json".into(),
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
                    DialogView::Help | DialogView::About => {
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
                        SetDlgItemTextW(hwnd, 112, w!("42")).unwrap();
                        for page in 0..5 {
                            SendDlgItemMessageW(
                                hwnd,
                                settings_ui::TABS as i32,
                                windows::Win32::UI::Controls::TCM_SETCURSEL,
                                WPARAM(page),
                                LPARAM(0),
                            );
                            settings_ui::select_page(hwnd);
                        }
                        assert_eq!(
                            settings_ui::collect(hwnd, &snapshot)
                                .unwrap()
                                .default_awake_minutes,
                            42
                        );
                        settings_ui::reset_draft(hwnd, &snapshot);
                        assert_eq!(
                            settings_ui::collect(hwnd, &snapshot).unwrap(),
                            Settings::default()
                        );
                        SetDlgItemTextW(hwnd, 105, w!("0")).unwrap();
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

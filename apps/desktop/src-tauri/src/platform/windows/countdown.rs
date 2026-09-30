//! A standard Windows warning dialog; the rules worker remains the only power-action owner.
use super::dialog_template::Template;
use crate::{
    core::{countdown::Countdown, sessions::PowerAction},
    state::{Operation, Request},
};
use std::{
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            DialogBoxIndirectParamW, EndDialog, GetWindowLongPtrW, KillTimer, SetDlgItemTextW,
            SetTimer, SetWindowLongPtrW, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, WINDOW_LONG_PTR_INDEX, WM_CLOSE, WM_COMMAND, WM_INITDIALOG, WM_TIMER,
        },
    },
};

const STATUS: u16 = 130;
const SNOOZE: u16 = 131;
const USER_DATA: WINDOW_LONG_PTR_INDEX =
    WINDOW_LONG_PTR_INDEX((2 * std::mem::size_of::<isize>()) as i32);

#[derive(Clone, Copy)]
struct View {
    action: PowerAction,
    remaining: u64,
    preview: bool,
}
enum Update {
    State(Option<View>),
    Preview(PowerAction),
    Stop,
}

/// Owns a dedicated native message loop so an open settings dialog cannot delay the warning.
/// When hidden, this thread blocks on its channel and has no polling timer.
pub struct Warning {
    updates: Sender<Update>,
    thread: Option<JoinHandle<()>>,
}
impl Warning {
    pub fn new(requests: Sender<Request>) -> Result<Self, String> {
        let (updates, receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("doze-warning".into())
            .spawn(move || run(receiver, requests))
            .map_err(|error| error.to_string())?;
        Ok(Self {
            updates,
            thread: Some(thread),
        })
    }
    pub fn update(&self, countdown: Option<&Countdown>, now: u64) {
        let view = countdown.map(|countdown| View {
            action: countdown.action,
            remaining: countdown.deadline.saturating_sub(now),
            preview: false,
        });
        let _ = self.updates.send(Update::State(view));
    }
    pub fn preview(&self, action: PowerAction) {
        let _ = self.updates.send(Update::Preview(action));
    }
}
impl Drop for Warning {
    fn drop(&mut self) {
        let _ = self.updates.send(Update::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Dialog<'a> {
    updates: &'a Receiver<Update>,
    requests: &'a Sender<Request>,
    view: View,
    preview_deadline: Option<Instant>,
    active: bool,
    stopping: bool,
}
impl Dialog<'_> {
    fn apply(&mut self, update: Update) -> bool {
        match update {
            Update::Stop => {
                self.stopping = true;
                self.active = false;
            }
            Update::State(Some(view)) => {
                self.view = view;
                self.preview_deadline = None;
                self.active = true;
            }
            Update::State(None) => {
                self.active = self.view.preview;
            }
            Update::Preview(action) => {
                // A preview must never replace a real pending power action.
                if !self.active || self.view.preview {
                    self.view = View {
                        action,
                        remaining: 60,
                        preview: true,
                    };
                    self.preview_deadline = Some(Instant::now() + Duration::from_secs(60));
                    self.active = true;
                }
            }
        }
        self.active
    }
    fn command(&mut self, id: u16) -> Result<bool, String> {
        if self.view.preview {
            if id == SNOOZE {
                self.preview_deadline = self
                    .preview_deadline
                    .map(|deadline| deadline + Duration::from_secs(900));
            }
            return Ok(id != SNOOZE);
        }
        let operation = if id == SNOOZE {
            Operation::Snooze
        } else {
            Operation::Cancel
        };
        let (reply, _) = mpsc::channel();
        self.requests
            .send(Request::Operation(operation, reply))
            .map(|_| false)
            .map_err(|_| "Doze engine stopped.".into())
    }
}

fn run(updates: Receiver<Update>, requests: Sender<Request>) {
    while let Ok(update) = updates.recv() {
        let view = match update {
            Update::State(Some(view)) => view,
            Update::Preview(action) => View {
                action,
                remaining: 60,
                preview: true,
            },
            Update::State(None) => continue,
            Update::Stop => break,
        };
        let mut dialog = Dialog {
            updates: &updates,
            requests: &requests,
            view,
            preview_deadline: view
                .preview
                .then(|| Instant::now() + Duration::from_secs(60)),
            active: true,
            stopping: false,
        };
        // Discard obsolete states before opening a window (for example, a just-cancelled countdown).
        let mut active = true;
        while let Ok(update) = updates.try_recv() {
            active = dialog.apply(update);
            if dialog.stopping {
                break;
            }
        }
        if dialog.stopping {
            break;
        }
        if !active {
            continue;
        }
        let template = template();
        // The template and borrowed channel context outlive the modal message loop.
        let result = unsafe {
            DialogBoxIndirectParamW(
                HINSTANCE::default(),
                template.as_ptr().cast(),
                HWND::default(),
                Some(dialog_proc),
                LPARAM((&mut dialog as *mut Dialog<'_>) as isize),
            )
        };
        if result == -1 {
            let error = windows::core::Error::from_win32().to_string();
            let _ = requests.send(Request::WarningFailed(error));
        }
        if dialog.stopping {
            break;
        }
    }
}

fn template() -> Vec<u32> {
    let mut form = Template::new("Doze · Power countdown", 278, 106);
    form.label_with_id(STATUS, "", 12, 14, 252);
    form.label("Cancel the action or snooze for 15 minutes.", 12, 38, 252);
    form.button(2, "Cancel", 202, 78, true);
    form.button(SNOOZE, "Snooze 15 min", 130, 78, false);
    form.finish()
}

unsafe fn render(hwnd: HWND, dialog: &mut Dialog<'_>) {
    if let Some(deadline) = dialog.preview_deadline {
        dialog.view.remaining = deadline.saturating_duration_since(Instant::now()).as_secs();
    }
    let view = dialog.view;
    let title = format!(
        "{} in {}:{:02}{}",
        view.action.label(),
        view.remaining / 60,
        view.remaining % 60,
        if view.preview { " (preview)" } else { "" }
    );
    let text: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    let _ = SetDlgItemTextW(hwnd, STATUS as i32, PCWSTR(text.as_ptr()));
}

unsafe extern "system" fn dialog_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    if message == WM_INITDIALOG {
        SetWindowLongPtrW(hwnd, USER_DATA, lparam.0);
        let dialog = &mut *(lparam.0 as *mut Dialog<'_>);
        render(hwnd, dialog);
        // This timer only drains rendering updates; it never executes a power action.
        if SetTimer(hwnd, 1, 250, None) == 0 {
            let _ = dialog.requests.send(Request::WarningFailed(
                "Could not update the native countdown window.".into(),
            ));
            let _ = EndDialog(hwnd, 0);
        }
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        return 1;
    }
    if !matches!(message, WM_TIMER | WM_CLOSE | WM_COMMAND) {
        return 0;
    }
    let context = GetWindowLongPtrW(hwnd, USER_DATA) as *mut Dialog<'_>;
    if context.is_null() {
        return 0;
    }
    let dialog = &mut *context;
    if message == WM_TIMER {
        let mut active = true;
        loop {
            match dialog.updates.try_recv() {
                Ok(update) => {
                    active = dialog.apply(update);
                    if dialog.stopping {
                        break;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    active = false;
                    dialog.stopping = true;
                    break;
                }
            }
        }
        if !active
            || dialog.stopping
            || dialog
                .preview_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
        {
            let _ = KillTimer(hwnd, 1);
            let _ = EndDialog(hwnd, 0);
        } else {
            render(hwnd, dialog);
        }
        return 1;
    }
    let id = (wparam.0 & 0xffff) as u16;
    if message == WM_CLOSE || (message == WM_COMMAND && (id == 2 || id == SNOOZE)) {
        if !matches!(
            dialog.command(if message == WM_CLOSE { 2 } else { id }),
            Ok(false)
        ) {
            let _ = KillTimer(hwnd, 1);
            let _ = EndDialog(hwnd, 0);
        }
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateDialogIndirectParamW, DestroyWindow, GetDlgItemTextW,
    };

    fn context<'a>(
        updates: &'a Receiver<Update>,
        requests: &'a Sender<Request>,
        preview: bool,
    ) -> Dialog<'a> {
        Dialog {
            updates,
            requests,
            view: View {
                action: PowerAction::Sleep,
                remaining: 65,
                preview,
            },
            preview_deadline: preview.then(|| Instant::now() + Duration::from_secs(60)),
            active: true,
            stopping: false,
        }
    }
    #[test]
    fn real_warning_buttons_submit_cancel_and_snooze_to_the_engine() {
        let (_, updates) = mpsc::channel();
        let (requests, operations) = mpsc::channel();
        let mut dialog = context(&updates, &requests, false);
        assert!(!dialog.command(2).unwrap());
        assert!(matches!(
            operations.try_recv().unwrap(),
            Request::Operation(Operation::Cancel, _)
        ));
        assert!(!dialog.command(SNOOZE).unwrap());
        assert!(matches!(
            operations.try_recv().unwrap(),
            Request::Operation(Operation::Snooze, _)
        ));
        assert!(!dialog.apply(Update::State(None)));
    }
    #[test]
    fn preview_buttons_never_submit_power_operations() {
        let (_, updates) = mpsc::channel();
        let (requests, operations) = mpsc::channel();
        let mut dialog = context(&updates, &requests, true);
        let before = dialog.preview_deadline.unwrap();
        assert!(!dialog.command(SNOOZE).unwrap());
        assert_eq!(
            dialog.preview_deadline.unwrap().duration_since(before),
            Duration::from_secs(900)
        );
        assert!(dialog.command(2).unwrap());
        assert!(matches!(operations.try_recv(), Err(TryRecvError::Empty)));
        assert!(!dialog.stopping); // Closing a preview leaves the warning service available.
    }
    #[test]
    fn real_countdown_overrides_preview_and_ignores_new_previews() {
        let (_, updates) = mpsc::channel();
        let (requests, _) = mpsc::channel();
        let mut dialog = context(&updates, &requests, true);
        dialog.apply(Update::State(Some(View {
            action: PowerAction::Lock,
            remaining: 120,
            preview: false,
        })));
        dialog.apply(Update::Preview(PowerAction::Shutdown));
        assert_eq!(dialog.view.action, PowerAction::Lock);
        assert!(dialog.preview_deadline.is_none());
        assert!(!dialog.apply(Update::State(None)));
        assert!(dialog.apply(Update::Preview(PowerAction::Sleep)));
        assert!(dialog.view.preview);
    }
    unsafe extern "system" fn test_proc(_: HWND, _: u32, _: WPARAM, _: LPARAM) -> isize {
        0
    }
    #[test]
    fn windows_creates_warning_controls_and_renders_remaining_time() {
        let (_, updates) = mpsc::channel();
        let (requests, _) = mpsc::channel();
        let mut dialog = context(&updates, &requests, false);
        let template = template();
        // Hidden native construction test; no message loop, notification or power action.
        let hwnd = unsafe {
            CreateDialogIndirectParamW(
                HINSTANCE::default(),
                template.as_ptr().cast(),
                HWND::default(),
                Some(test_proc),
                LPARAM(0),
            )
        }
        .unwrap();
        unsafe {
            render(hwnd, &mut dialog);
            let mut text = [0u16; 100];
            let length = GetDlgItemTextW(hwnd, STATUS as i32, &mut text);
            assert_eq!(
                String::from_utf16_lossy(&text[..length as usize]),
                "Sleep in 1:05"
            );
            DestroyWindow(hwnd).unwrap();
        }
    }
}

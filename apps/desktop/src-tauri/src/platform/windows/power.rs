use crate::{core::sessions::PowerAction, platform::PowerManager};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use windows::{
    core::{w, GUID, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, GetLastError, HANDLE, LPARAM, LUID, WPARAM},
        Security::{
            AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES,
            SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
        },
        System::{
            Power::*,
            Shutdown::{
                InitiateSystemShutdownExW, LockWorkStation, SHTDN_REASON_FLAG_PLANNED,
                SHTDN_REASON_MAJOR_APPLICATION,
            },
            Threading::{GetCurrentProcess, OpenProcessToken},
        },
        UI::WindowsAndMessaging::{
            SendMessageTimeoutW, DEVICE_NOTIFY_CALLBACK, HWND_BROADCAST, PBT_POWERSETTINGCHANGE,
            SC_MONITORPOWER, SMTO_ABORTIFHUNG, WM_SYSCOMMAND,
        },
    },
};

pub struct NativePower {
    held: bool,
    allow_display_sleep: bool,
    notice: Option<String>,
}

const AWAY_MODE_NOTICE: &str = "Windows stayed on in Away Mode instead of sleeping because another app requested it (powercfg /requests names it) · desktop locked";

// Windows reports entering (1) and leaving (0) Away Mode through this power setting.
const SYSTEM_AWAYMODE: GUID = GUID::from_u128(0x98a7f580_01f7_48aa_9c0f_44352c29e5c0);
static ENTERED_AWAY_MODE: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn away_mode_changed(
    _context: *const c_void,
    kind: u32,
    setting: *const c_void,
) -> u32 {
    if kind == PBT_POWERSETTINGCHANGE && !setting.is_null() {
        let setting = &*(setting as *const POWERBROADCAST_SETTING);
        if setting.PowerSetting == SYSTEM_AWAYMODE
            && setting.DataLength >= 1
            && setting.Data[0] == 1
        {
            ENTERED_AWAY_MODE.store(true, Ordering::SeqCst);
        }
    }
    0
}

/// Watches, while it lives, for Windows turning a sleep request into Away Mode: when an app
/// asks for Away Mode and the power plan allows it, the screen goes dark and sound mutes but
/// the computer keeps running, so nothing asks for a password on return. Requests made with
/// PowerSetRequest (as Logitech G HUB does) do not appear in the system execution state, so
/// only this notification is reliable.
pub(super) struct AwayModeWatch {
    handle: *mut c_void,
    parameters: Option<Box<DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS>>,
}

impl AwayModeWatch {
    pub(super) fn start() -> Option<Self> {
        ENTERED_AWAY_MODE.store(false, Ordering::SeqCst);
        let parameters = Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(away_mode_changed),
            Context: std::ptr::null_mut(),
        });
        let mut handle = std::ptr::null_mut();
        unsafe {
            PowerSettingRegisterNotification(
                &SYSTEM_AWAYMODE,
                DEVICE_NOTIFY_CALLBACK,
                HANDLE(
                    (&*parameters as *const DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS)
                        .cast_mut()
                        .cast(),
                ),
                &mut handle,
            )
            .ok()
            .ok()?;
        }
        Some(Self {
            handle,
            parameters: Some(parameters),
        })
    }

    /// Whether Windows entered Away Mode, waiting briefly for the asynchronous notification.
    pub(super) fn entered(&self, wait: Duration) -> bool {
        let deadline = Instant::now() + wait;
        loop {
            if ENTERED_AWAY_MODE.load(Ordering::SeqCst) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for AwayModeWatch {
    fn drop(&mut self) {
        let result =
            unsafe { PowerSettingUnregisterNotification(HPOWERNOTIFY(self.handle as isize)) };
        if result.is_err() {
            // A failed unregister may leave the callback alive; keep its parameters until exit.
            if let Some(parameters) = self.parameters.take() {
                Box::leak(parameters);
            }
        }
    }
}

fn execution_flags(active: bool, allow_display_sleep: bool) -> EXECUTION_STATE {
    if !active {
        return ES_CONTINUOUS;
    }
    let flags = ES_CONTINUOUS | ES_SYSTEM_REQUIRED;
    if allow_display_sleep {
        flags
    } else {
        flags | ES_DISPLAY_REQUIRED
    }
}
impl NativePower {
    pub fn new() -> Self {
        Self {
            held: false,
            allow_display_sleep: false,
            notice: None,
        }
    }
}

// Execution state belongs to this worker thread; acquisition and release never move threads.
impl PowerManager for NativePower {
    fn describe(&self) -> Vec<String> {
        if !self.held {
            return Vec::new();
        }
        let mut requests = vec!["System required · ES_SYSTEM_REQUIRED | ES_CONTINUOUS".to_string()];
        if !self.allow_display_sleep {
            requests.push("Display required · ES_DISPLAY_REQUIRED".into());
        }
        requests
    }
    fn supported_actions(&self) -> Vec<PowerAction> {
        let mut actions = vec![
            PowerAction::Lock,
            PowerAction::DisplayOff,
            PowerAction::Shutdown,
        ];
        let mut caps = SYSTEM_POWER_CAPABILITIES::default();
        unsafe {
            if GetPwrCapabilities(&mut caps).as_bool() {
                if caps.SystemS1.as_bool()
                    || caps.SystemS2.as_bool()
                    || caps.SystemS3.as_bool()
                    || caps.AoAc.as_bool()
                {
                    actions.insert(0, PowerAction::Sleep);
                }
                if caps.SystemS4.as_bool() && caps.HiberFilePresent.as_bool() {
                    actions.insert(1.min(actions.len()), PowerAction::Hibernate);
                }
            }
        }
        actions
    }
    fn set_awake(&mut self, active: bool, allow_display_sleep: bool) -> Result<(), String> {
        if active == self.held && (!active || allow_display_sleep == self.allow_display_sleep) {
            return Ok(());
        }
        let flags = execution_flags(active, allow_display_sleep);
        if unsafe { SetThreadExecutionState(flags) }.0 == 0 {
            return Err("Windows could not update the power request.".into());
        }
        self.held = active;
        self.allow_display_sleep = allow_display_sleep;
        Ok(())
    }
    fn execute(&mut self, action: PowerAction) -> Result<(), String> {
        if !self.supported_actions().contains(&action) {
            return Err("This power action is not supported on this computer.".into());
        }
        self.set_awake(false, false)?;
        self.notice = None;
        unsafe {
            match action {
                PowerAction::Sleep | PowerAction::Hibernate => {
                    // Lock first, so the desktop is locked on return even when Windows uses
                    // Away Mode or does not require sign-in after sleep. Locking is
                    // asynchronous; give the lock screen a moment before suspending.
                    if LockWorkStation().is_ok() {
                        std::thread::sleep(std::time::Duration::from_millis(1500));
                    }
                    let watch = (action == PowerAction::Sleep)
                        .then(AwayModeWatch::start)
                        .flatten();
                    let result = with_shutdown_privilege(|| {
                        if SetSuspendState(action == PowerAction::Hibernate, false, false).as_bool()
                        {
                            Ok(())
                        } else {
                            Err(format!("Windows refused suspend: {:?}", GetLastError()))
                        }
                    });
                    if result.is_ok()
                        && watch
                            .as_ref()
                            .is_some_and(|watch| watch.entered(Duration::from_secs(3)))
                    {
                        self.notice = Some(AWAY_MODE_NOTICE.into());
                    }
                    result
                }
                PowerAction::Shutdown => with_shutdown_privilege(|| {
                    InitiateSystemShutdownExW(
                        PCWSTR::null(),
                        w!("Doze scheduled shutdown"),
                        0,
                        false,
                        false,
                        SHTDN_REASON_FLAG_PLANNED | SHTDN_REASON_MAJOR_APPLICATION,
                    )
                    .map_err(|e| e.to_string())
                }),
                PowerAction::Lock => LockWorkStation().map_err(|e| e.to_string()),
                PowerAction::DisplayOff => {
                    if SendMessageTimeoutW(
                        HWND_BROADCAST,
                        WM_SYSCOMMAND,
                        WPARAM(SC_MONITORPOWER as usize),
                        LPARAM(2),
                        SMTO_ABORTIFHUNG,
                        2000,
                        None,
                    )
                    .0 == 0
                    {
                        Err("Windows did not acknowledge the display-off request.".into())
                    } else {
                        Ok(())
                    }
                }
            }
        }
    }
    fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }
}
impl Drop for NativePower {
    fn drop(&mut self) {
        let _ = self.set_awake(false, false);
    }
}

unsafe fn with_shutdown_privilege(
    action: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let mut token = HANDLE::default();
    OpenProcessToken(
        GetCurrentProcess(),
        TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
        &mut token,
    )
    .map_err(|e| e.to_string())?;
    let result = (|| {
        let mut luid = LUID::default();
        LookupPrivilegeValueW(PCWSTR::null(), w!("SeShutdownPrivilege"), &mut luid)
            .map_err(|e| e.to_string())?;
        let requested = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let mut previous = TOKEN_PRIVILEGES::default();
        let mut previous_length = 0;
        // Windows requires ReturnLength when PreviousState is supplied.
        AdjustTokenPrivileges(
            token,
            false,
            Some(&requested),
            std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
            Some(&mut previous),
            Some(&mut previous_length),
        )
        .map_err(|e| e.to_string())?;
        let error = GetLastError();
        if error.0 != 0 {
            return Err(format!("Shutdown privilege unavailable: {error:?}"));
        }
        let result = action();
        let _ = AdjustTokenPrivileges(token, false, Some(&previous), 0, None, None);
        result
    })();
    let _ = CloseHandle(token);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shutdown_privilege_wrapper_reaches_action_without_executing_power_change() {
        let mut called = false;
        let result = unsafe {
            with_shutdown_privilege(|| {
                called = true;
                Ok(())
            })
        };
        match result {
            Ok(()) => assert!(called),
            // Restricted test tokens can lack this privilege. The Windows call must
            // still succeed, report that restriction, and never invoke the action.
            Err(error)
                if error
                    == format!(
                        "Shutdown privilege unavailable: {:?}",
                        windows::Win32::Foundation::ERROR_NOT_ALL_ASSIGNED
                    ) =>
            {
                assert!(!called);
            }
            Err(error) => panic!("Shutdown privilege setup failed: {error}"),
        }
    }

    #[test]
    fn away_mode_notifications_are_recognised() {
        // Registering delivers the current value, which is not Away Mode while testing.
        let watch = AwayModeWatch::start().expect("Away Mode notifications are available");
        assert!(!watch.entered(Duration::from_millis(300)));
        let entering = POWERBROADCAST_SETTING {
            PowerSetting: SYSTEM_AWAYMODE,
            DataLength: 1,
            Data: [1],
        };
        let other = POWERBROADCAST_SETTING {
            PowerSetting: GUID::zeroed(),
            ..entering
        };
        let leaving = POWERBROADCAST_SETTING {
            Data: [0],
            ..entering
        };
        unsafe {
            for setting in [&other, &leaving] {
                away_mode_changed(
                    std::ptr::null(),
                    PBT_POWERSETTINGCHANGE,
                    (setting as *const POWERBROADCAST_SETTING).cast(),
                );
            }
            assert!(!watch.entered(Duration::ZERO));
            away_mode_changed(
                std::ptr::null(),
                PBT_POWERSETTINGCHANGE,
                (&entering as *const POWERBROADCAST_SETTING).cast(),
            );
        }
        assert!(watch.entered(Duration::ZERO));
        drop(watch);
        // A new watch starts clean.
        assert!(!AwayModeWatch::start().unwrap().entered(Duration::ZERO));
    }

    #[test]
    fn notices_are_reported_once() {
        let mut power = NativePower::new();
        assert_eq!(power.take_notice(), None);
        power.notice = Some(AWAY_MODE_NOTICE.into());
        assert_eq!(power.take_notice().as_deref(), Some(AWAY_MODE_NOTICE));
        assert_eq!(power.take_notice(), None);
    }

    #[test]
    fn allowing_display_sleep_preserves_the_system_awake_request() {
        let flags = execution_flags(true, true);
        assert_ne!(flags.0 & ES_SYSTEM_REQUIRED.0, 0);
        assert_eq!(flags.0 & ES_DISPLAY_REQUIRED.0, 0);
        assert_ne!(execution_flags(true, false).0 & ES_DISPLAY_REQUIRED.0, 0);
        assert_eq!(execution_flags(false, false), ES_CONTINUOUS);
    }
}

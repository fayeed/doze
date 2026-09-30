use crate::{core::sessions::PowerAction, platform::PowerManager};
use windows::{
    core::{w, PCWSTR},
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
            SendMessageTimeoutW, HWND_BROADCAST, SC_MONITORPOWER, SMTO_ABORTIFHUNG, WM_SYSCOMMAND,
        },
    },
};

pub struct NativePower {
    held: bool,
}
impl NativePower {
    pub fn new() -> Self {
        Self { held: false }
    }
}

// Execution state belongs to this worker thread; acquisition and release never move threads.
impl PowerManager for NativePower {
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
    fn set_awake(&mut self, active: bool) -> Result<(), String> {
        if active == self.held {
            return Ok(());
        }
        let flags = if active {
            ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
        } else {
            ES_CONTINUOUS
        };
        if unsafe { SetThreadExecutionState(flags) }.0 == 0 {
            return Err("Windows could not update the power request.".into());
        }
        self.held = active;
        Ok(())
    }
    fn execute(&mut self, action: PowerAction) -> Result<(), String> {
        if !self.supported_actions().contains(&action) {
            return Err("This power action is not supported on this computer.".into());
        }
        self.set_awake(false)?;
        unsafe {
            match action {
                PowerAction::Sleep | PowerAction::Hibernate => with_shutdown_privilege(|| {
                    if SetSuspendState(action == PowerAction::Hibernate, false, false).as_bool() {
                        Ok(())
                    } else {
                        Err(format!("Windows refused suspend: {:?}", GetLastError()))
                    }
                }),
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
}
impl Drop for NativePower {
    fn drop(&mut self) {
        let _ = self.set_awake(false);
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
        AdjustTokenPrivileges(
            token,
            false,
            Some(&requested),
            std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
            Some(&mut previous),
            None,
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

use crate::{core::sessions::PowerAction, platform::PowerManager};
use std::{
    ffi::{c_char, c_int, c_void},
    process::Command,
};
type CFString = *const c_void;
extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}
const RTLD_LAZY: c_int = 1;

/// The login framework's immediate lock is what the system Lock Screen command uses. It is
/// resolved at runtime and Lock is only offered when it exists.
fn lock_screen() -> Option<unsafe extern "C" fn() -> c_int> {
    unsafe {
        let framework = dlopen(
            c"/System/Library/PrivateFrameworks/login.framework/Versions/Current/login".as_ptr(),
            RTLD_LAZY,
        );
        if framework.is_null() {
            return None;
        }
        let symbol = dlsym(framework, c"SACLockScreenImmediate".as_ptr());
        (!symbol.is_null()).then(|| std::mem::transmute(symbol))
    }
}

fn run(program: &str, args: &[&str], action: &str) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("Could not {action}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    if detail.contains("-1743") {
        return Err(format!(
            "macOS blocked {action}. Allow Doze in System Settings › Privacy & Security › Automation."
        ));
    }
    Err(format!("macOS refused to {action}: {}", detail.trim()))
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        text: *const c_char,
        encoding: u32,
    ) -> CFString;
    fn CFRelease(value: *const c_void);
}
#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPMAssertionCreateWithName(kind: CFString, level: u32, name: CFString, id: *mut u32)
        -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
    fn IOPMFindPowerManagement(port: u32) -> u32;
    fn IOPMSleepSystem(connection: u32) -> i32;
    fn IOServiceClose(connection: u32) -> i32;
}
pub struct NativePower {
    assertion: Option<u32>,
    allow_display_sleep: bool,
}
impl NativePower {
    pub fn new() -> Self {
        Self {
            assertion: None,
            allow_display_sleep: false,
        }
    }
}
impl PowerManager for NativePower {
    fn describe(&self) -> Vec<String> {
        self.assertion
            .map(|id| {
                let kind = if self.allow_display_sleep {
                    "PreventUserIdleSystemSleep"
                } else {
                    "PreventUserIdleDisplaySleep"
                };
                format!("{kind} · “Doze keep awake” · IOKit assertion {id}")
            })
            .into_iter()
            .collect()
    }
    fn supported_actions(&self) -> Vec<PowerAction> {
        let mut actions = vec![PowerAction::Sleep, PowerAction::Shutdown];
        if lock_screen().is_some() {
            actions.push(PowerAction::Lock);
        }
        actions.push(PowerAction::DisplayOff);
        actions
    }
    fn set_awake(&mut self, active: bool, allow_display_sleep: bool) -> Result<(), String> {
        if active && self.assertion.is_some() && allow_display_sleep != self.allow_display_sleep {
            self.set_awake(false, false)?;
        }
        unsafe {
            if active && self.assertion.is_none() {
                let kind = CFStringCreateWithCString(
                    std::ptr::null(),
                    if allow_display_sleep {
                        c"PreventUserIdleSystemSleep".as_ptr()
                    } else {
                        c"PreventUserIdleDisplaySleep".as_ptr()
                    },
                    0x08000100,
                );
                let name = CFStringCreateWithCString(
                    std::ptr::null(),
                    c"Doze keep awake".as_ptr(),
                    0x08000100,
                );
                if kind.is_null() || name.is_null() {
                    if !kind.is_null() {
                        CFRelease(kind);
                    }
                    if !name.is_null() {
                        CFRelease(name);
                    }
                    return Err("Could not allocate power assertion.".into());
                }
                let mut id = 0;
                let status = IOPMAssertionCreateWithName(kind, 255, name, &mut id);
                CFRelease(kind);
                CFRelease(name);
                if status != 0 {
                    return Err(format!("IOKit assertion failed: {status}"));
                }
                self.assertion = Some(id);
                self.allow_display_sleep = allow_display_sleep;
            } else if !active {
                if let Some(id) = self.assertion {
                    let status = IOPMAssertionRelease(id);
                    if status != 0 {
                        return Err(format!("IOKit release failed: {status}"));
                    }
                    self.assertion = None;
                }
            }
        }
        Ok(())
    }
    fn execute(&mut self, action: PowerAction) -> Result<(), String> {
        if !self.supported_actions().contains(&action) {
            return Err("This power action is not supported on this Mac.".into());
        }
        self.set_awake(false, false)?;
        match action {
            PowerAction::Sleep => unsafe {
                let connection = IOPMFindPowerManagement(0);
                if connection == 0 {
                    return Err("Could not connect to IOKit power management.".into());
                }
                let status = IOPMSleepSystem(connection);
                IOServiceClose(connection);
                if status == 0 {
                    Ok(())
                } else {
                    Err(format!("macOS refused sleep: {status}"))
                }
            },
            // The standard shut down request lets apps with unsaved documents stop it, as on
            // Windows. Nothing is force-quit.
            PowerAction::Shutdown => run(
                "/usr/bin/osascript",
                &["-e", "tell application \"loginwindow\" to «event aevtshut»"],
                "shut down",
            ),
            // Its return value is undocumented, so only its availability is checked.
            PowerAction::Lock => match lock_screen() {
                Some(lock) => {
                    unsafe { lock() };
                    Ok(())
                }
                None => Err("Screen locking is unavailable on this Mac.".into()),
            },
            PowerAction::DisplayOff => run(
                "/usr/bin/pmset",
                &["displaysleepnow"],
                "turn the display off",
            ),
            PowerAction::Hibernate => Err("macOS does not offer hibernation to apps.".into()),
        }
    }
}
impl Drop for NativePower {
    fn drop(&mut self) {
        let _ = self.set_awake(false, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_actions_never_include_hibernate_and_unsupported_actions_are_refused() {
        let mut power = NativePower::new();
        let actions = power.supported_actions();
        assert_eq!(&actions[..2], &[PowerAction::Sleep, PowerAction::Shutdown]);
        assert!(actions.contains(&PowerAction::DisplayOff));
        assert!(!actions.contains(&PowerAction::Hibernate));
        assert!(power.execute(PowerAction::Hibernate).is_err());
    }
}

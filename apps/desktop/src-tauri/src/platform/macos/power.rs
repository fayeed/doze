use crate::{core::sessions::PowerAction, platform::PowerManager};
use std::ffi::{c_char, c_void};
type CFString = *const c_void;
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
}
impl NativePower {
    pub fn new() -> Self {
        Self { assertion: None }
    }
}
impl PowerManager for NativePower {
    fn supported_actions(&self) -> Vec<PowerAction> {
        vec![PowerAction::Sleep]
    }
    fn set_awake(&mut self, active: bool) -> Result<(), String> {
        unsafe {
            if active && self.assertion.is_none() {
                let kind = CFStringCreateWithCString(
                    std::ptr::null(),
                    c"PreventUserIdleDisplaySleep".as_ptr(),
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
        if action != PowerAction::Sleep {
            return Err("Unsupported macOS power action.".into());
        }
        self.set_awake(false)?;
        unsafe {
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
        }
    }
}
impl Drop for NativePower {
    fn drop(&mut self) {
        let _ = self.set_awake(false);
    }
}

//! Stay awake with the lid closed. A closed MacBook sleeps whatever power assertions say,
//! unless an external display is attached on power. While Doze keeps the Mac awake it sets the
//! power manager's clamshell override, the switch powerd itself sets for closed-display mode,
//! which needs no administrator rights. powerd clears it when displays, the power source or the
//! system's sleep state change, so a thread sets it again every few seconds while Doze holds
//! the Mac. The engine's checks can be a minute apart, and its clock stops while the Mac sleeps.
//!
//! Clearing it while an external display is attached could leave closed-display mode without
//! the override powerd set, so Doze waits until no external display is attached. A record file
//! marks the override as Doze's, so the next launch clears one left behind by a crash.
use std::{
    ffi::{c_char, c_void},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, Weak},
    time::Duration,
};

type CFStringRef = *const c_void;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceMatching(name: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> u32;
    fn IOServiceOpen(service: u32, task: u32, kind: u32, connection: *mut u32) -> i32;
    fn IOServiceClose(connection: u32) -> i32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IOConnectCallScalarMethod(
        connection: u32,
        selector: u32,
        input: *const u64,
        input_count: u32,
        output: *mut u64,
        output_count: *mut u32,
    ) -> i32;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: CFStringRef,
        allocator: *const c_void,
        options: u32,
    ) -> *const c_void;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        text: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    fn CFRelease(value: *const c_void);
}
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGGetOnlineDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayIsBuiltin(display: u32) -> u32;
}
extern "C" {
    static mach_task_self_: u32;
}

/// IOPMrootDomain's user client method for the clamshell override (IOPMLibDefs.h).
const SET_CLAMSHELL_SLEEP_STATE: u32 = 12;
const UTF8: u32 = 0x0800_0100;
/// How often a held override is set again, in case powerd cleared it.
const REFRESH: Duration = Duration::from_secs(2);

/// The power manager, released by the caller.
fn root_domain() -> Option<u32> {
    let service =
        unsafe { IOServiceGetMatchingService(0, IOServiceMatching(c"IOPMrootDomain".as_ptr())) };
    (service != 0).then_some(service)
}

/// Whether this Mac has a lid: the power manager reports a clamshell state only on laptops.
pub fn supported() -> bool {
    let Some(root) = root_domain() else {
        return false;
    };
    unsafe {
        let key =
            CFStringCreateWithCString(std::ptr::null(), c"AppleClamshellState".as_ptr(), UTF8);
        let value = if key.is_null() {
            std::ptr::null()
        } else {
            IORegistryEntryCreateCFProperty(root, key, std::ptr::null(), 0)
        };
        if !key.is_null() {
            CFRelease(key);
        }
        IOObjectRelease(root);
        if value.is_null() {
            return false;
        }
        CFRelease(value);
        true
    }
}

/// Sets or clears the override that keeps a closed lid from sleeping the Mac.
fn set_override(keep_awake: bool) -> Result<(), String> {
    let root = root_domain().ok_or("Could not reach the power manager.")?;
    unsafe {
        let mut connection = 0;
        let status = IOServiceOpen(root, mach_task_self_, 0, &mut connection);
        IOObjectRelease(root);
        if status != 0 {
            return Err(format!("Could not open the power manager: {status:#x}"));
        }
        let input = u64::from(keep_awake);
        let status = IOConnectCallScalarMethod(
            connection,
            SET_CLAMSHELL_SLEEP_STATE,
            &input,
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        IOServiceClose(connection);
        if status != 0 {
            return Err(format!("Could not change lid sleep: {status:#x}"));
        }
    }
    Ok(())
}

/// Whether a display other than the built-in one is attached.
fn external_display() -> bool {
    let mut displays = [0u32; 16];
    let mut count = 0;
    unsafe {
        CGGetOnlineDisplayList(displays.len() as u32, displays.as_mut_ptr(), &mut count) == 0
            && displays[..count as usize]
                .iter()
                .any(|&display| CGDisplayIsBuiltin(display) == 0)
    }
}

pub struct Lid {
    record: PathBuf,
    /// Whether Doze holds the override. The refresh thread sets it again only while this is
    /// true, under the same lock, so it never sets it after Doze has cleared it.
    held: Arc<Mutex<bool>>,
    refreshing: bool,
    /// The override may still be set: Doze let go with an external display attached, or a
    /// crash left the record behind.
    pending_clear: bool,
}

/// Sets a held override again every few seconds, until the `Lid` is dropped.
fn refresh(held: Weak<Mutex<bool>>) {
    loop {
        std::thread::sleep(REFRESH);
        let Some(held) = held.upgrade() else {
            return;
        };
        if *lock(&held) {
            let _ = set_override(true);
        }
    }
}

fn lock(held: &Mutex<bool>) -> MutexGuard<'_, bool> {
    held.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Lid {
    pub fn new(record: PathBuf) -> Self {
        let pending_clear = record.exists();
        let mut lid = Self {
            record,
            held: Arc::new(Mutex::new(false)),
            refreshing: false,
            pending_clear,
        };
        let _ = lid.set(false);
        lid
    }

    /// Keeps a closed lid from sleeping the Mac while `hold` is true; restores normal lid
    /// sleep otherwise, once no external display is attached.
    pub fn set(&mut self, hold: bool) -> Result<(), String> {
        let mut held = lock(&self.held);
        if hold {
            if *held {
                return Ok(());
            }
            std::fs::write(&self.record, b"{\"clamshellOverride\":true}\n")
                .map_err(|e| format!("Could not save the lid setting: {e}"))?;
            set_override(true)?;
            *held = true;
            self.pending_clear = false;
            if !self.refreshing {
                let weak = Arc::downgrade(&self.held);
                self.refreshing = std::thread::Builder::new()
                    .name("doze-lid".into())
                    .spawn(move || refresh(weak))
                    .is_ok();
            }
            return Ok(());
        }
        if std::mem::take(&mut *held) {
            self.pending_clear = true;
        }
        if self.pending_clear && !external_display() {
            set_override(false)?;
            let _ = std::fs::remove_file(&self.record);
            self.pending_clear = false;
        }
        Ok(())
    }

    /// For Settings › Advanced › Show active assertions.
    pub fn describe(&self) -> Option<String> {
        lock(&self.held).then(|| {
            "Lid close: stays awake (normal lid sleep returns when Doze lets go)".to_string()
        })
    }
}

impl Drop for Lid {
    fn drop(&mut self) {
        let _ = self.set(false);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Sets and clears the Mac's clamshell override"]
    fn holding_and_releasing_the_lid_override() {
        assert!(super::supported(), "This test needs a MacBook.");
        let record = std::env::temp_dir().join(format!("doze-lid-{}.json", uuid::Uuid::new_v4()));
        let mut lid = super::Lid::new(record.clone());
        lid.set(true).unwrap();
        assert!(record.exists());
        assert!(lid.describe().is_some());
        lid.set(false).unwrap();
        assert!(lid.describe().is_none());
        assert_eq!(record.exists(), super::external_display());
    }
}

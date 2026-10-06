//! Battery level for the battery guard (IOKit power sources), and process names for tools
//! detected by process (libproc).
use std::ffi::{c_char, c_int, c_void};

type CFTypeRef = *const c_void;
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(
        allocator: CFTypeRef,
        text: *const c_char,
        encoding: u32,
    ) -> CFTypeRef;
    fn CFRelease(value: CFTypeRef);
    fn CFEqual(a: CFTypeRef, b: CFTypeRef) -> u8;
    fn CFArrayGetCount(array: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
    fn CFDictionaryGetValue(dictionary: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    fn CFNumberGetValue(number: CFTypeRef, kind: isize, value: *mut c_void) -> u8;
}
#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPSCopyPowerSourcesInfo() -> CFTypeRef;
    fn IOPSCopyPowerSourcesList(blob: CFTypeRef) -> CFTypeRef;
    fn IOPSGetPowerSourceDescription(blob: CFTypeRef, source: CFTypeRef) -> CFTypeRef;
}
extern "C" {
    fn proc_listallpids(buffer: *mut c_void, size: c_int) -> c_int;
    fn proc_name(pid: c_int, buffer: *mut c_void, size: u32) -> c_int;
}
const UTF8: u32 = 0x0800_0100;
const NUMBER_SINT32: isize = 3;

/// An owned CFString, released on drop.
struct Text(CFTypeRef);
impl Text {
    fn new(text: &std::ffi::CStr) -> Option<Self> {
        let value = unsafe { CFStringCreateWithCString(std::ptr::null(), text.as_ptr(), UTF8) };
        (!value.is_null()).then_some(Self(value))
    }
}
impl Drop for Text {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}

fn number(dictionary: CFTypeRef, key: &Text) -> Option<i32> {
    let value = unsafe { CFDictionaryGetValue(dictionary, key.0) };
    if value.is_null() {
        return None;
    }
    let mut out: i32 = 0;
    (unsafe { CFNumberGetValue(value, NUMBER_SINT32, (&mut out as *mut i32).cast()) } != 0)
        .then_some(out)
}

/// Percent and whether the Mac runs on battery; None without an internal battery.
pub fn read() -> Option<(u8, bool)> {
    let kind_key = Text::new(c"Type")?;
    let internal = Text::new(c"InternalBattery")?;
    let current_key = Text::new(c"Current Capacity")?;
    let max_key = Text::new(c"Max Capacity")?;
    let state_key = Text::new(c"Power Source State")?;
    let on_battery = Text::new(c"Battery Power")?;
    unsafe {
        let blob = IOPSCopyPowerSourcesInfo();
        if blob.is_null() {
            return None;
        }
        let list = IOPSCopyPowerSourcesList(blob);
        let mut result = None;
        if !list.is_null() {
            for index in 0..CFArrayGetCount(list) {
                let source = CFArrayGetValueAtIndex(list, index);
                let description = IOPSGetPowerSourceDescription(blob, source);
                if description.is_null() {
                    continue;
                }
                let kind = CFDictionaryGetValue(description, kind_key.0);
                if kind.is_null() || CFEqual(kind, internal.0) == 0 {
                    continue;
                }
                let (Some(current), Some(max)) = (
                    number(description, &current_key),
                    number(description, &max_key),
                ) else {
                    continue;
                };
                let state = CFDictionaryGetValue(description, state_key.0);
                let battery = !state.is_null() && CFEqual(state, on_battery.0) != 0;
                let percent = if max > 0 {
                    (current.clamp(0, max) * 100 / max) as u8
                } else {
                    continue;
                };
                result = Some((percent, battery));
                break;
            }
            CFRelease(list);
        }
        CFRelease(blob);
        result
    }
}

pub fn processes() -> Vec<String> {
    let mut pids = vec![0 as c_int; 4096];
    let bytes = (pids.len() * std::mem::size_of::<c_int>()) as c_int;
    let count = unsafe { proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
    if count <= 0 {
        return Vec::new();
    }
    pids.truncate(count as usize);
    let mut names = Vec::new();
    let mut buffer = [0u8; 256];
    for pid in pids {
        let length = unsafe { proc_name(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
        if length > 0 {
            names.push(String::from_utf8_lossy(&buffer[..length as usize]).into_owned());
        }
    }
    names
}

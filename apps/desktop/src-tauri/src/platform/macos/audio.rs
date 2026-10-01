//! Playback detection through Core Audio object state. No audio is captured or inspected, so
//! no recording permission is needed: an output device counts as playing while Core Audio
//! reports it running, it is not muted and its volume is above zero. On macOS 14+, the process
//! list additionally requires some process to be running output, so microphone-only use of a
//! headset does not count. Silent streams that keep a device running do count; this is the
//! main difference from Windows peak meters.
use crate::platform::AudioMonitor;
use std::ffi::c_void;

type ObjectId = u32;
#[repr(C)]
struct Address {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectHasProperty(object: ObjectId, address: *const Address) -> u8;
    fn AudioObjectGetPropertyDataSize(
        object: ObjectId,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> i32;
    fn AudioObjectGetPropertyData(
        object: ObjectId,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> i32;
}

const fn code(text: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*text)
}
const SYSTEM: ObjectId = 1;
const GLOBAL: u32 = code(b"glob");
const OUTPUT: u32 = code(b"outp");
const MAIN: u32 = 0;
const DEVICES: u32 = code(b"dev#");
const DEFAULT_OUTPUT: u32 = code(b"dOut");
const PROCESSES: u32 = code(b"prs#");
const PROCESS_RUNNING_OUTPUT: u32 = code(b"piro");
const STREAMS: u32 = code(b"stm#");
const RUNNING_SOMEWHERE: u32 = code(b"gone");
const MUTE: u32 = code(b"mute");
const VIRTUAL_MAIN_VOLUME: u32 = code(b"vmvc");
const VOLUME_SCALAR: u32 = code(b"volm");

fn address(selector: u32, scope: u32) -> Address {
    Address {
        selector,
        scope,
        element: MAIN,
    }
}

fn has(object: ObjectId, selector: u32, scope: u32) -> bool {
    unsafe { AudioObjectHasProperty(object, &address(selector, scope)) != 0 }
}

fn list(object: ObjectId, selector: u32, scope: u32) -> Result<Vec<ObjectId>, String> {
    let address = address(selector, scope);
    let mut size = 0;
    let status =
        unsafe { AudioObjectGetPropertyDataSize(object, &address, 0, std::ptr::null(), &mut size) };
    if status != 0 {
        return Err(format!("Core Audio error {status}"));
    }
    let mut items = vec![0 as ObjectId; size as usize / size_of::<ObjectId>()];
    if items.is_empty() {
        return Ok(items);
    }
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            items.as_mut_ptr().cast(),
        )
    };
    if status != 0 {
        return Err(format!("Core Audio error {status}"));
    }
    items.truncate(size as usize / size_of::<ObjectId>());
    Ok(items)
}

fn value<T: Copy + Default>(object: ObjectId, selector: u32, scope: u32) -> Result<T, String> {
    let mut data = T::default();
    let mut size = size_of::<T>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &address(selector, scope),
            0,
            std::ptr::null(),
            &mut size,
            (&mut data as *mut T).cast(),
        )
    };
    if status == 0 {
        Ok(data)
    } else {
        Err(format!("Core Audio error {status}"))
    }
}

fn optional<T: Copy + Default>(object: ObjectId, selector: u32, scope: u32) -> Option<T> {
    has(object, selector, scope)
        .then(|| value(object, selector, scope).ok())
        .flatten()
}

fn outputs() -> Result<Vec<ObjectId>, String> {
    Ok(list(SYSTEM, DEVICES, GLOBAL)?
        .into_iter()
        .filter(|device| list(*device, STREAMS, OUTPUT).is_ok_and(|s| !s.is_empty()))
        .collect())
}

/// Muted or zero-volume output is silence, as on Windows. Devices without software volume
/// (some HDMI and aggregate devices) are treated as audible.
fn audible(device: ObjectId) -> bool {
    if optional::<u32>(device, MUTE, OUTPUT).is_some_and(|muted| muted != 0) {
        return false;
    }
    optional::<f32>(device, VIRTUAL_MAIN_VOLUME, OUTPUT)
        .or_else(|| optional::<f32>(device, VOLUME_SCALAR, OUTPUT))
        .is_none_or(|volume| volume > 0.001)
}

/// Whether any process is running audio output. `None` before macOS 14.
fn process_output() -> Option<bool> {
    if !has(SYSTEM, PROCESSES, GLOBAL) {
        return None;
    }
    let processes = list(SYSTEM, PROCESSES, GLOBAL).ok()?;
    Some(processes.into_iter().any(|process| {
        optional::<u32>(process, PROCESS_RUNNING_OUTPUT, GLOBAL).is_some_and(|running| running != 0)
    }))
}

pub struct NativeAudio {
    devices: Vec<ObjectId>,
    default_output: ObjectId,
}

impl NativeAudio {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            devices: outputs()?,
            default_output: value(SYSTEM, DEFAULT_OUTPUT, GLOBAL)?,
        })
    }
}

impl AudioMonitor for NativeAudio {
    fn sample(&mut self) -> Result<bool, String> {
        let devices = outputs()?;
        let default_output = value(SYSTEM, DEFAULT_OUTPUT, GLOBAL)?;
        if devices != self.devices || default_output != self.default_output {
            self.devices = devices;
            self.default_output = default_output;
            return Err("Audio devices changed; waiting for fresh playback.".into());
        }
        if self.devices.is_empty() {
            return Err("No active audio output device.".into());
        }
        if process_output() == Some(false) {
            return Ok(false);
        }
        Ok(self.devices.iter().any(|device| {
            optional::<u32>(*device, RUNNING_SOMEWHERE, GLOBAL).is_some_and(|running| running != 0)
                && audible(*device)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Read-only probe requires macOS audio hardware"]
    fn native_read_only_probe() -> Result<(), String> {
        let mut audio = NativeAudio::new()?;
        for device in &audio.devices {
            println!(
                "Output {device}: running {:?}, muted {:?}, volume {:?}, audible {}",
                optional::<u32>(*device, RUNNING_SOMEWHERE, GLOBAL),
                optional::<u32>(*device, MUTE, OUTPUT),
                optional::<f32>(*device, VIRTUAL_MAIN_VOLUME, OUTPUT),
                audible(*device)
            );
        }
        println!("Process output: {:?}", process_output());
        println!("Meaningful audio active: {}", audio.sample()?);
        Ok(())
    }
}

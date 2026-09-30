use crate::platform::{IdleMonitor, IdleObservation};
use windows::Win32::{
    System::SystemInformation::GetTickCount,
    UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
};
pub struct NativeIdle;
impl IdleMonitor for NativeIdle {
    fn observe(&self) -> Result<IdleObservation, String> {
        let mut input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        unsafe {
            if !GetLastInputInfo(&mut input).as_bool() {
                return Err("Could not read user idle time.".into());
            }
            Ok(IdleObservation {
                seconds: GetTickCount().wrapping_sub(input.dwTime) as u64 / 1000,
                activity_marker: Some(input.dwTime as u64),
            })
        }
    }
}

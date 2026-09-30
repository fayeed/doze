use crate::platform::{IdleMonitor, IdleObservation};
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event: u32) -> f64;
}
pub struct NativeIdle;
impl IdleMonitor for NativeIdle {
    fn observe(&self) -> Result<IdleObservation, String> {
        let value = unsafe { CGEventSourceSecondsSinceLastEventType(1, u32::MAX) };
        if value.is_finite() && value >= 0.0 {
            Ok(IdleObservation {
                seconds: value as u64,
                activity_marker: None,
            })
        } else {
            Err("Could not read macOS idle time.".into())
        }
    }
}

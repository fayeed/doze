use crate::platform::AudioMonitor;
pub struct NativeAudio;
impl NativeAudio {
    pub fn new() -> Result<Self, String> {
        Err("macOS system audio monitoring is not implemented yet.".into())
    }
}
impl AudioMonitor for NativeAudio {
    fn sample(&mut self) -> Result<bool, String> {
        Err("macOS system audio monitoring is not implemented yet.".into())
    }
}

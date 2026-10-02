pub mod audio;
pub mod idle;
pub mod lifecycle;
pub mod power;
pub mod startup;

#[cfg(test)]
mod tests {
    use super::{audio::NativeAudio, idle::NativeIdle, power::NativePower};
    use crate::platform::{AudioMonitor, IdleMonitor, PowerManager};

    #[test]
    #[ignore = "Read-only probe requires Windows desktop hardware and audio endpoints"]
    fn native_read_only_probe() -> Result<(), String> {
        println!(
            "Available power actions: {:?}",
            NativePower::new().supported_actions()
        );
        println!("User idle: {} seconds", NativeIdle.observe()?.seconds);
        println!(
            "Away Mode notifications available: {}",
            super::power::AwayModeWatch::start().is_some()
        );
        let mut audio = NativeAudio::new()?;
        // The initial endpoint rebuild deliberately invalidates playback history.
        println!("Initial audio observation: {:?}", audio.sample());
        println!("Meaningful audio active: {}", audio.sample()?);
        Ok(())
    }
}
pub mod dialogs;
mod winui;

use super::{
    model::{Operation, Snapshot},
    persistence::persist,
};
use crate::{
    core::{countdown::Source, sessions::Settings},
    platform,
};

fn duration(seconds: u64) -> Result<u64, String> {
    if !(1..=7 * 24 * 3600).contains(&seconds) {
        Err("Choose a duration between 1 second and 7 days.".into())
    } else {
        Ok(seconds)
    }
}
pub(super) fn apply(
    op: Operation,
    snapshot: &mut Snapshot,
    path: &std::path::Path,
) -> Result<(), String> {
    match op {
        Operation::KeepAwake { seconds } => {
            if let Some(seconds) = seconds {
                duration(seconds)?;
            }
            snapshot.engine.keep_awake(seconds);
        }
        Operation::ExtendAwake { seconds } => {
            duration(seconds)?;
            snapshot.engine.extend_awake(seconds)?;
        }
        Operation::StopAwake => {
            snapshot.engine.stop_awake();
            snapshot.engine.while_audio = false;
        }

        Operation::ToggleWhileAudio => {
            if !snapshot.audio_supported {
                return Err("Audio monitoring is unavailable.".into());
            }
            snapshot.engine.while_audio = !snapshot.engine.while_audio;
        }
        Operation::TogglePlayback => {
            if !snapshot.audio_supported {
                return Err("After Playback is unavailable.".into());
            }
            snapshot
                .engine
                .enable_playback(!snapshot.engine.playback_enabled);
        }
        Operation::SelectAction { action } => {
            if !snapshot.actions.contains(&action) {
                return Err("Unsupported power action.".into());
            }
            snapshot.selected_action = action;
        }
        Operation::ScheduleSelected { seconds } => {
            duration(seconds)?;
            snapshot.engine.schedule(seconds, snapshot.selected_action);
        }
        Operation::Schedule { seconds, action } => {
            duration(seconds)?;
            if !snapshot.actions.contains(&action) {
                return Err("Unsupported power action.".into());
            }
            snapshot.engine.schedule(seconds, action);
        }
        Operation::StopTimer => {
            snapshot.engine.timer = None;
            if snapshot
                .engine
                .countdown
                .as_ref()
                .is_some_and(|c| c.source == Source::Timer)
            {
                snapshot.engine.cancel_countdown();
            }
        }

        Operation::Cancel => snapshot.engine.cancel_countdown(),
        Operation::Snooze => snapshot.engine.snooze()?,
        Operation::SaveSettings { settings } => save(snapshot, path, settings)?,
        Operation::ResetSettings => {
            let mut settings = Settings::default();
            if !snapshot.actions.contains(&settings.default_action) {
                let first = snapshot
                    .actions
                    .first()
                    .copied()
                    .ok_or("No power actions are available.")?;
                settings.default_action = first;
                settings.playback_action = first;
            }
            save(snapshot, path, settings)?;
        }
        Operation::OpenDialog { view } => snapshot.view = view,
        Operation::Refresh | Operation::PreviewCountdown | Operation::Quit => {}
    }
    Ok(())
}
fn save(snapshot: &mut Snapshot, path: &std::path::Path, settings: Settings) -> Result<(), String> {
    settings.validate()?;
    if !snapshot.actions.contains(&settings.default_action)
        || !snapshot.actions.contains(&settings.playback_action)
    {
        return Err("Choose a supported power action.".into());
    }
    let old = snapshot.settings.clone();
    if old.launch_at_startup != settings.launch_at_startup {
        platform::startup::set_enabled(settings.launch_at_startup)?;
    }
    if let Err(error) = persist(path, &settings) {
        if old.launch_at_startup != settings.launch_at_startup {
            let _ = platform::startup::set_enabled(old.launch_at_startup);
        }
        return Err(error);
    }
    snapshot.engine.cancel_playback();
    snapshot.selected_action = settings.default_action;
    snapshot.settings = settings;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_duration_bounds() {
        assert!(duration(0).is_err());
        assert!(duration(604801).is_err());
        assert!(duration(900).is_ok());
    }
    #[test]
    fn settings_validate_before_native_changes() {
        let settings = Settings {
            countdown_seconds: 0,
            ..Settings::default()
        };
        assert!(settings.validate().is_err());
    }
    #[test]
    fn settings_roundtrip_has_no_sessions() {
        let text = serde_json::to_string(&Settings::default()).unwrap();
        assert!(!text.contains("deadline"));
        assert_eq!(
            serde_json::from_str::<Settings>(&text).unwrap(),
            Settings::default()
        );
    }
}

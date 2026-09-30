use super::{
    model::{Operation, Preference, Snapshot},
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
        Operation::KeepAwakeDefault => snapshot
            .engine
            .keep_awake(Some(snapshot.settings.default_awake_minutes * 60)),
        Operation::ScheduleDefault => snapshot.engine.schedule(
            snapshot.settings.default_timer_minutes * 60,
            snapshot.selected_action,
        ),
        Operation::TogglePreference { preference } => {
            let mut settings = snapshot.settings.clone();
            let value = match preference {
                Preference::AllowDisplaySleep => &mut settings.allow_display_sleep,
                Preference::Notifications => &mut settings.notifications,
                Preference::LaunchAtStartup => &mut settings.launch_at_startup,
                Preference::StartMinimized => &mut settings.start_minimized,
                Preference::Logging => &mut settings.logging,
            };
            *value = !*value;
            save(snapshot, path, settings)?;
        }
        Operation::SetDefaultDuration { awake, minutes } => {
            let mut settings = snapshot.settings.clone();
            if awake {
                settings.default_awake_minutes = minutes;
            } else {
                settings.default_timer_minutes = minutes;
            }
            save(snapshot, path, settings)?;
        }
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
    if old.playback_action != settings.playback_action
        || old.silence_seconds != settings.silence_seconds
        || old.idle_seconds != settings.idle_seconds
    {
        snapshot.engine.cancel_playback();
    }
    if old.default_action != settings.default_action {
        snapshot.selected_action = settings.default_action;
    }
    snapshot.settings = settings;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_settings_files_receive_new_session_defaults() {
        let settings: Settings = serde_json::from_str("{\"notifications\":false}").unwrap();
        assert!(!settings.notifications);
        assert_eq!(settings.default_awake_minutes, 30);
        assert_eq!(settings.default_timer_minutes, 30);
        assert!(!settings.allow_display_sleep);
    }
    #[test]
    fn quick_preferences_preserve_live_sessions_and_action_selection() {
        use crate::core::sessions::{Engine, PowerAction};
        let test_dir =
            std::env::temp_dir().join(format!("doze-preferences-{}", std::process::id()));
        let path = test_dir.join("settings.json");
        let mut snapshot = Snapshot {
            settings_path: path.clone(),
            engine: Engine::default(),
            settings: Settings::default(),
            actions: vec![PowerAction::Sleep, PowerAction::Lock],
            audio_supported: true,
            startup_supported: true,
            error: None,
            selected_action: PowerAction::Lock,
            view: super::super::model::DialogView::Settings,
        };
        snapshot.engine.enable_playback(true);
        for time in 0..3 {
            snapshot
                .engine
                .tick(time, Some(true), Some(600), false, &snapshot.settings);
        }
        snapshot.engine.schedule(900, PowerAction::Lock);
        let deadline = snapshot.engine.timer.as_ref().unwrap().deadline;
        apply(
            Operation::TogglePreference {
                preference: Preference::Notifications,
            },
            &mut snapshot,
            &path,
        )
        .unwrap();
        assert_eq!(
            snapshot.engine.playback_phase,
            crate::core::sessions::Phase::Active
        );
        assert_eq!(snapshot.selected_action, PowerAction::Lock);
        apply(
            Operation::SetDefaultDuration {
                awake: false,
                minutes: 60,
            },
            &mut snapshot,
            &path,
        )
        .unwrap();
        assert_eq!(snapshot.engine.timer.as_ref().unwrap().deadline, deadline);
        apply(Operation::ScheduleDefault, &mut snapshot, &path).unwrap();
        assert_eq!(
            snapshot.engine.timer.as_ref().unwrap().deadline,
            snapshot.engine.now + 3600
        );
        let saved = super::super::persistence::load(&path).unwrap();
        assert_eq!(saved.default_timer_minutes, 60);
        assert!(!saved.notifications);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&test_dir).unwrap();
    }
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

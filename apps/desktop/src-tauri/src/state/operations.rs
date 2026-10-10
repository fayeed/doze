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
    if matches!(
        op,
        Operation::CancelAgent { .. } | Operation::WaitAgent { .. } | Operation::FinishAgent { .. }
    ) {
        let (id, status) = match op {
            Operation::CancelAgent { id } => (id, crate::mcp::sessions::Status::Cancelled),
            Operation::WaitAgent { id } => (id, crate::mcp::sessions::Status::Active),
            Operation::FinishAgent { id } => (id, crate::mcp::sessions::Status::Finished),
            _ => return Err("Invalid agent operation.".into()),
        };
        let session = snapshot
            .engine
            .agents
            .items
            .iter_mut()
            .find(|s| s.session_id == id)
            .ok_or("Session not found.")?;
        if session.status.terminal() {
            return Err("Session already ended.".into());
        }
        if status != crate::mcp::sessions::Status::Cancelled && !session.status.holds_awake() {
            return Err("Session not authorized.".into());
        }
        session.status = status;
        if status == crate::mcp::sessions::Status::Active {
            session.lease_expires_at = snapshot.engine.now.saturating_add(1800);
            session.timeout_at = None;
        }
        return Ok(());
    }
    if matches!(
        op,
        Operation::KeepAwake { .. } | Operation::KeepAwakeDefault | Operation::StayAwake
    ) && snapshot.engine.battery_low
    {
        return Err(format!(
            "Battery is below {}%. Plug in to keep awake.",
            snapshot.settings.battery_floor_percent
        ));
    }
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

        Operation::Cancel => snapshot.engine.dismiss_countdown(),
        Operation::StayAwake => {
            snapshot.engine.dismiss_countdown();
            let minutes = snapshot.settings.stay_awake_minutes;
            snapshot.engine.keep_awake(minutes.map(|m| m * 60));
        }
        Operation::Snooze => snapshot
            .engine
            .snooze(snapshot.settings.snooze_minutes * 60)?,
        Operation::SaveSettings { mut settings } => {
            // Native general settings edits cannot overwrite credentials or agent permissions.
            settings.agents = snapshot.settings.agents.clone();
            save(snapshot, path, settings)?;
        }
        Operation::OpenDialog { view } => {
            snapshot.view = view;
            snapshot.page = None;
        }
        Operation::OpenPage { page } => {
            snapshot.view = super::DialogView::Settings;
            snapshot.page = Some(page);
        }
        Operation::OpenPanel { anchor } => {
            snapshot.view = super::DialogView::Panel;
            snapshot.panel_anchor = Some(anchor);
        }
        Operation::Set { key, value } => {
            let settings = set(&snapshot.settings, &key, value)?;
            save(snapshot, path, settings)?;
        }
        Operation::ConnectPreview { agent, remove } => {
            let home = crate::agents::connect::home()?;
            let executable = crate::agents::connect::executable();
            let change = crate::agents::connect::preview(&agent, remove, &home, &executable)?;
            snapshot.result = Some(serde_json::json!(change));
        }
        Operation::ConnectApply {
            agent,
            remove,
            token,
        } => {
            let home = crate::agents::connect::home()?;
            let executable = crate::agents::connect::executable();
            let note = crate::agents::connect::apply(&agent, remove, &token, &home, &executable)?;
            // Removing an agent also forgets that it was allowed.
            if remove {
                let mut settings = snapshot.settings.clone();
                settings.agents.trusted.retain(|id| id != &agent);
                save(snapshot, path, settings)?;
            }
            snapshot.engine.message = Some(note.clone());
            snapshot.result = Some(serde_json::json!({ "message": note }));
        }
        Operation::CopyConfig => {
            let mut settings = snapshot.settings.clone();
            crate::mcp::tools::connect(&mut settings, "Generic MCP client")?;
            if settings != snapshot.settings {
                save(snapshot, path, settings)?;
            }
            let configs =
                crate::mcp::server::connection_configs(&snapshot.settings, &snapshot.settings_path);
            let config = configs
                .as_array()
                .into_iter()
                .flatten()
                .find(|c| c["name"] == "Generic MCP client")
                .and_then(|c| c["generic"].as_str())
                .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                .ok_or("Could not create the MCP configuration.")?;
            let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
            snapshot.result = Some(serde_json::json!({ "config": text }));
        }
        Operation::ExportDiagnostics { path: target } => {
            let report = super::diagnostics::report(snapshot);
            std::fs::write(
                &target,
                serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("Could not save the report: {e}"))?;
            snapshot.engine.message = Some("Diagnostics report saved".into());
        }
        Operation::Reset => {
            let mut settings = Settings::default();
            // Keep connected tools working; everything they were allowed to do is reset.
            settings.agents.clients = snapshot.settings.agents.clients.clone();
            for client in &mut settings.agents.clients {
                client.keep_awake = false;
                client.actions.clear();
            }
            if let Some(first) = snapshot.actions.first().copied() {
                if !snapshot.actions.contains(&settings.default_action) {
                    settings.default_action = first;
                }
                if !snapshot.actions.contains(&settings.playback_action) {
                    settings.playback_action = first;
                }
            }
            save(snapshot, path, settings)?;
        }
        Operation::InstallAgentSkill { name, update } => {
            snapshot.engine.message = Some(crate::mcp::skill::install(&name, update)?);
        }
        Operation::OpenAgentSkillFolder => crate::mcp::skill::open_folder(path)?,
        Operation::ConnectAgent { name } => {
            let mut settings = snapshot.settings.clone();
            crate::mcp::tools::connect(&mut settings, &name)?;
            save(snapshot, path, settings)?;
            snapshot.view = super::DialogView::Agents;
        }
        Operation::AuthorizeAgent { id, decision } => {
            let mut engine = snapshot.engine.clone();
            let mut settings = snapshot.settings.clone();
            crate::mcp::tools::authorize(&mut engine, &mut settings, &id, &decision)?;
            save(snapshot, path, settings)?;
            snapshot.engine = engine;
        }
        Operation::CancelAgent { id }
        | Operation::WaitAgent { id }
        | Operation::FinishAgent { id } => {
            // Handled below with the operation discriminator.
            return Err(format!("Unexpected agent operation for {id}"));
        }
        Operation::AgentEnabled => {
            let mut settings = snapshot.settings.clone();
            settings.agents.enabled = !settings.agents.enabled;
            save(snapshot, path, settings)?;
        }
        Operation::RevokeAgent { id } => {
            let mut settings = snapshot.settings.clone();
            settings.agents.clients.retain(|c| c.id != id);
            save(snapshot, path, settings)?;
            for session in &mut snapshot.engine.agents.items {
                if session.client_id == id && !session.status.terminal() {
                    session.status = crate::mcp::sessions::Status::Cancelled;
                }
            }
            snapshot.engine.agents.completion_consumed = true;
            if snapshot
                .engine
                .countdown
                .as_ref()
                .is_some_and(|c| c.source == Source::Agents)
            {
                snapshot.engine.cancel_countdown();
            }
        }
        Operation::AgentKeepAlive => {
            let mut settings = snapshot.settings.clone();
            settings.agents.keep_alive_while_connected =
                !settings.agents.keep_alive_while_connected;
            save(snapshot, path, settings)?;
        }
        Operation::AgentLease { seconds } => {
            let mut settings = snapshot.settings.clone();
            settings.agents.lease_seconds = seconds;
            save(snapshot, path, settings)?;
        }
        Operation::AgentDefault { action } => {
            let mut settings = snapshot.settings.clone();
            settings.agents.default_completion = action;
            save(snapshot, path, settings)?;
        }
        Operation::AgentPermission { id, action } => {
            let mut settings = snapshot.settings.clone();
            let client = settings
                .agents
                .clients
                .iter_mut()
                .find(|c| c.id == id)
                .ok_or("Client not found.")?;
            if let Some(action) = action {
                if client.actions.contains(&action) {
                    client.actions.retain(|a| *a != action);
                } else {
                    client.actions.push(action);
                }
            } else {
                client.keep_awake = !client.keep_awake;
            }
            save(snapshot, path, settings)?;
            // Permission changes never leave an already authorized automatic action behind.
            for session in &mut snapshot.engine.agents.items {
                if session.client_id == id && session.status.holds_awake() {
                    session.status = crate::mcp::sessions::Status::Cancelled;
                }
            }
            snapshot.engine.agents.completion_consumed = true;
            if snapshot
                .engine
                .countdown
                .as_ref()
                .is_some_and(|c| c.source == Source::Agents)
            {
                snapshot.engine.cancel_countdown();
            }
        }
        Operation::Refresh | Operation::PreviewCountdown | Operation::Quit => {}
    }
    Ok(())
}

/// Saved keys no UI may set directly: agent credentials are created and revoked only
/// through their own operations.
const PROTECTED: [&str; 2] = ["agents", "agents.clients"];

/// Changes one setting by its saved key. Only existing keys are accepted and the result
/// must deserialize and validate like a settings file.
fn set(settings: &Settings, key: &str, value: serde_json::Value) -> Result<Settings, String> {
    if PROTECTED.contains(&key) || key.starts_with("agents.clients.") {
        return Err("Agent credentials change only through Connect and Remove.".into());
    }
    let mut document = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    let mut target = &mut document;
    let parts: Vec<&str> = key.split('.').collect();
    for part in &parts {
        target = target
            .as_object_mut()
            .and_then(|object| object.get_mut(*part))
            .ok_or_else(|| format!("Unknown setting \"{key}\"."))?;
    }
    *target = value;
    serde_json::from_value(document).map_err(|e| format!("Invalid value for \"{key}\": {e}"))
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
    let agents_off = old.agents.enabled && !settings.agents.enabled;
    let mcp_off = old.mcp_server_enabled && !settings.mcp_server_enabled;
    snapshot.settings = settings;
    if agents_off || mcp_off {
        // Command-line jobs are the user's own and depend on neither setting. Turning MCP off
        // ends MCP leases; turning agents off ends every agent session.
        for session in &mut snapshot.engine.agents.items {
            let mcp = session.source == crate::agents::SessionSource::McpLease;
            if !session.status.terminal()
                && session.client_id != crate::mcp::sessions::LOCAL_CLIENT_ID
                && (agents_off || mcp)
            {
                session.status = crate::mcp::sessions::Status::Cancelled;
            }
        }
        snapshot.engine.agents.completion_consumed = true;
        if snapshot
            .engine
            .countdown
            .as_ref()
            .is_some_and(|c| c.source == Source::Agents)
        {
            snapshot.engine.cancel_countdown();
        }
    }
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
        assert_eq!(settings.theme, crate::core::sessions::Theme::System);
    }
    #[test]
    fn appearance_settings_roundtrip_and_reject_unknown_values() {
        use crate::core::sessions::Theme;
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            let settings = Settings {
                theme,
                ..Settings::default()
            };
            let text = serde_json::to_string(&settings).unwrap();
            assert_eq!(
                serde_json::from_str::<Settings>(&text).unwrap().theme,
                theme
            );
        }
        assert!(serde_json::from_str::<Settings>(r#"{"theme":"invalid"}"#).is_err());
    }
    #[test]
    fn quick_preferences_preserve_live_sessions_and_action_selection() {
        use crate::core::sessions::PowerAction;
        let test_dir =
            std::env::temp_dir().join(format!("doze-preferences-{}", std::process::id()));
        let path = test_dir.join("settings.json");
        let mut snapshot = Snapshot {
            actions: vec![PowerAction::Sleep, PowerAction::Lock],
            selected_action: PowerAction::Lock,
            ..Snapshot::new(path.clone(), Settings::default())
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
    fn set_changes_one_saved_key_and_protects_credentials() {
        let mut fixture = AgentFixture::new();
        let path = fixture.snapshot.settings_path.clone();
        let mut set = |key: &str, value: serde_json::Value| {
            apply(
                Operation::Set {
                    key: key.into(),
                    value,
                },
                &mut fixture.snapshot,
                &path,
            )
        };
        set("snoozeMinutes", serde_json::json!(5)).unwrap();
        set("stayAwakeMinutes", serde_json::json!(60)).unwrap();
        set("stayAwakeMinutes", serde_json::Value::Null).unwrap();
        set("agents.askBeforeNew", serde_json::json!(false)).unwrap();
        assert!(set("snoozeMinutes", serde_json::json!(0)).is_err());
        assert!(set("snoozeMinutes", serde_json::Value::Null).is_err());
        assert!(set("noSuchSetting", serde_json::json!(true)).is_err());
        assert!(set("agents.clients", serde_json::json!([])).is_err());
        assert!(set("agents", serde_json::json!({})).is_err());
        let saved = super::super::persistence::load(&path).unwrap();
        assert_eq!(saved.snooze_minutes, 5);
        assert_eq!(saved.stay_awake_minutes, None);
        assert!(!saved.agents.ask_before_new);
        assert_eq!(saved.agents.clients.len(), 1);
        // Snooze follows the saved setting.
        fixture.snapshot.engine = crate::core::sessions::Engine::default();
        fixture
            .snapshot
            .engine
            .schedule(1, crate::core::sessions::PowerAction::Sleep);
        let s = fixture.snapshot.settings.clone();
        fixture.snapshot.engine.tick(1, None, None, false, &s);
        let deadline = fixture
            .snapshot
            .engine
            .countdown
            .as_ref()
            .map(|c| c.deadline);
        fixture.apply(Operation::Snooze);
        assert_eq!(
            fixture
                .snapshot
                .engine
                .countdown
                .as_ref()
                .map(|c| c.deadline),
            deadline.map(|d| d + 300)
        );
    }
    #[test]
    fn turning_agents_off_ends_their_sessions_without_completion() {
        let mut fixture = AgentFixture::new();
        let path = fixture.snapshot.settings_path.clone();
        apply(
            Operation::Set {
                key: "agents.enabled".into(),
                value: serde_json::json!(false),
            },
            &mut fixture.snapshot,
            &path,
        )
        .unwrap();
        assert!(!fixture.snapshot.engine.should_hold_awake());
        let s = fixture.snapshot.settings.clone();
        assert_eq!(
            fixture.snapshot.engine.tick(1000, None, None, false, &s),
            None
        );
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
    struct AgentFixture {
        snapshot: Snapshot,
        directory: std::path::PathBuf,
    }
    impl AgentFixture {
        fn new() -> Self {
            use crate::core::sessions::{Engine, PowerAction};
            let directory =
                std::env::temp_dir().join(format!("doze-agents-{}", uuid::Uuid::new_v4()));
            let path = directory.join("settings.json");
            let mut settings = Settings::default();
            settings.agents.enabled = true;
            crate::mcp::tools::connect(&mut settings, "Codex").unwrap();
            settings.agents.clients[0].keep_awake = true;
            settings.agents.clients[0].actions = vec![PowerAction::Sleep];
            let mut engine = Engine::default();
            crate::mcp::tools::call(
                &mut engine,
                &settings,
                &[PowerAction::Sleep],
                crate::mcp::tools::Call {
                    key: settings.agents.clients[0].secret.clone(),
                    name: "doze.start_session".into(),
                    arguments: serde_json::json!({"reason":"tests", "completion_action":"sleep"}),
                },
            )
            .unwrap();
            Self {
                snapshot: Snapshot {
                    engine,
                    actions: vec![PowerAction::Sleep],
                    audio_supported: false,
                    startup_supported: false,
                    ..Snapshot::new(path, settings)
                },
                directory,
            }
        }
        fn apply(&mut self, op: Operation) {
            let path = self.snapshot.settings_path.clone();
            apply(op, &mut self.snapshot, &path).unwrap();
        }
    }
    impl Drop for AgentFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
    #[test]
    fn general_settings_and_reset_preserve_client_permissions() {
        let mut fixture = AgentFixture::new();
        let expected = fixture.snapshot.settings.agents.clone();
        fixture.apply(Operation::SaveSettings {
            settings: Settings::default(),
        });
        assert_eq!(fixture.snapshot.settings.agents, expected);
        assert!(fixture.snapshot.engine.should_hold_awake());
    }
    #[test]
    fn disabling_mcp_cancels_leases_without_completion() {
        let mut fixture = AgentFixture::new();
        fixture.apply(Operation::AgentEnabled);
        assert!(!fixture.snapshot.settings.agents.enabled);
        assert!(!fixture.snapshot.engine.should_hold_awake());
        let s = fixture.snapshot.settings.clone();
        assert_eq!(
            fixture.snapshot.engine.tick(1000, None, None, false, &s),
            None
        );
    }
    #[test]
    fn revoked_credentials_and_active_sessions_cannot_continue() {
        let mut fixture = AgentFixture::new();
        let client = fixture.snapshot.settings.agents.clients[0].clone();
        fixture.apply(Operation::RevokeAgent { id: client.id });
        assert!(fixture
            .snapshot
            .settings
            .agents
            .authenticate(&client.secret)
            .is_err());
        assert!(!fixture.snapshot.engine.should_hold_awake());
        assert!(fixture.snapshot.engine.agents.completion_consumed);
    }
    #[test]
    fn permission_reduction_cancels_authorized_sessions() {
        let mut fixture = AgentFixture::new();
        let id = fixture.snapshot.settings.agents.clients[0].id.clone();
        fixture.apply(Operation::AgentPermission {
            id,
            action: Some(crate::core::sessions::PowerAction::Sleep),
        });
        assert!(fixture.snapshot.settings.agents.clients[0]
            .actions
            .is_empty());
        assert!(!fixture.snapshot.engine.should_hold_awake());
    }
    #[test]
    fn wait_thirty_minutes_and_manual_resolution_preserve_authorization() {
        let mut fixture = AgentFixture::new();
        let settings = fixture.snapshot.settings.clone();
        fixture
            .snapshot
            .engine
            .tick(600, None, None, false, &settings);
        let id = fixture.snapshot.engine.agents.items[0].session_id.clone();
        fixture.apply(Operation::WaitAgent { id: id.clone() });
        assert_eq!(
            fixture.snapshot.engine.agents.items[0].lease_expires_at,
            2400
        );
        fixture.apply(Operation::FinishAgent { id });
        fixture
            .snapshot
            .engine
            .tick(601, None, None, false, &settings);
        assert!(fixture.snapshot.engine.countdown.is_some());
        fixture.apply(Operation::StayAwake);
        assert!(fixture.snapshot.engine.countdown.is_none());
        assert!(fixture.snapshot.engine.awake);
        assert_eq!(
            fixture
                .snapshot
                .engine
                .tick(5000, None, None, false, &settings),
            None
        );
    }
}

use super::{
    model::{Operation, Request, Snapshot},
    operations::apply,
};
use crate::core::countdown::Source;
use crate::platform::{self, AudioMonitor, IdleMonitor, NotificationManager, PowerManager};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant, SystemTime},
};

/// One-off failures stay visible this long, then the status shows the session state again.
const RECENT_ERROR_SECONDS: u64 = 300;

/// The error shown in the tray and Settings, grouped by how long each kind stays true.
/// Startup conditions persist, observation failures last while they recur, a settings
/// load failure lasts until settings are saved, and one-off failures expire.
#[derive(Default)]
struct Errors {
    startup: Option<String>,
    settings: Option<String>,
    observation: Option<String>,
    agents: Option<String>,
    recent: Option<(String, u64)>,
}
impl Errors {
    fn report(&mut self, error: String, now: u64) {
        self.recent = Some((error, now.saturating_add(RECENT_ERROR_SECONDS)));
    }
    /// Agent bridge failures only matter to people who enabled MCP.
    fn current(&mut self, now: u64, agents_enabled: bool) -> Option<String> {
        if self.recent.as_ref().is_some_and(|(_, until)| now >= *until) {
            self.recent = None;
        }
        self.recent
            .as_ref()
            .map(|(error, _)| error)
            .or(self.observation.as_ref())
            .or(self.startup.as_ref())
            .or(self.agents.as_ref().filter(|_| agents_enabled))
            .or(self.settings.as_ref())
            .cloned()
    }
    /// The worker wakes when a recent error expires so the tray stops showing it.
    fn expires_in(&self, now: u64) -> Option<Duration> {
        self.recent
            .as_ref()
            .map(|(_, until)| Duration::from_secs(until.saturating_sub(now).max(1)))
    }
}

/// Whether the engine may have been suspended or frozen since its last iteration.
/// Deadlines use the monotonic clock, so wall-clock changes alone (time sync, manual
/// changes, time zones) never affect sessions. They are only a suspend heuristic when
/// native suspend notifications are unavailable. A stalled worker always counts, since
/// a deadline may have passed without its visible countdown.
fn discontinuity(
    elapsed: Duration,
    wall_elapsed: Option<Duration>,
    wait: Duration,
    suspend_notifications: bool,
) -> bool {
    let overslept = elapsed > wait + Duration::from_secs(10);
    let clock_jumped = wall_elapsed.is_none_or(|w| w.abs_diff(elapsed) > Duration::from_secs(5));
    overslept || (!suspend_notifications && clock_jumped)
}

type CountdownKey = (u64, Source);

/// Whether a countdown needs a native notification now, and whether it is the only warning.
/// Optional notifications are attempted once per countdown. Without its warning window a
/// countdown still runs only once Windows has shown its notification; Cancel and Snooze
/// remain in the tray.
fn announcement(
    countdown: Option<CountdownKey>,
    attempted: Option<CountdownKey>,
    shown: Option<CountdownKey>,
    notifications: bool,
    window_failed: bool,
) -> Option<bool> {
    let countdown = countdown?;
    if window_failed {
        (shown != Some(countdown)).then_some(true)
    } else {
        (notifications && attempted != Some(countdown)).then_some(false)
    }
}

pub(super) fn worker(
    app: tauri::AppHandle,
    path: PathBuf,
    mut snapshot: Snapshot,
    receiver: Receiver<Request>,
    sender: Sender<Request>,
) {
    let mut power = platform::NativePower::new();
    // Restores a lid setting left changed by a crash before anything else runs.
    let mut lid = platform::Lid::new(path.with_file_name("lid-restore.json"));
    let idle = platform::NativeIdle;
    let notifications = platform::NativeNotifications(app.clone());
    let mut errors = Errors {
        settings: snapshot.error.take(),
        ..Errors::default()
    };
    let warning = platform::countdown::Warning::new(sender.clone());
    if let Err(error) = &warning {
        errors.startup = Some(format!("Countdown window unavailable: {error}"));
    }
    // The login item follows the saved setting, even after settings were reset or the app
    // moved: re-register the current executable, or remove a stale item.
    if let Err(error) = platform::startup::set_enabled(snapshot.settings.launch_at_startup) {
        errors.startup = Some(error);
    }
    let lifecycle = platform::lifecycle::Registration::new(sender.clone());
    if let Err(error) = &lifecycle {
        errors
            .startup
            .get_or_insert(format!("Suspend notifications unavailable: {error}"));
    }
    snapshot.actions = power.supported_actions();
    if let Some(first) = snapshot.actions.first().copied() {
        if !snapshot.actions.contains(&snapshot.settings.default_action) {
            snapshot.settings.default_action = first;
        }
        if !snapshot
            .actions
            .contains(&snapshot.settings.playback_action)
        {
            snapshot.settings.playback_action = first;
        }
    }
    let mut audio = None;
    let mut retry_audio_at = 0;
    let origin = Instant::now();
    let mut previous_time = Instant::now();
    let mut previous_wall = SystemTime::now();
    let mut previous_idle = None;
    let mut previous_activity_marker = None;
    snapshot.selected_action = snapshot.settings.default_action;
    let mut logged_error = None;
    let mut attempted_notification = None;
    let mut shown_notification = None;
    let mut next_battery_at = 0;
    let mut next_scan_at = 0;
    loop {
        let now = origin.elapsed().as_secs();
        snapshot.engine.now = now;
        let engine = &snapshot.engine;
        let wait = if engine.needs_audio() || engine.countdown.is_some() {
            Duration::from_secs(1)
        } else if engine.completion_waiting(&snapshot.settings) {
            // Finished agents wait for the user to step away; check input now and then.
            Duration::from_secs(5)
        } else {
            let holding = engine.should_hold_awake() || engine.battery_low;
            let deadline = engine
                .awake_deadline
                .into_iter()
                .chain(engine.timer.as_ref().map(|t| t.deadline))
                .chain(engine.agents.next_deadline())
                // Tray labels show minutes, and the battery guard samples while holding.
                .chain(holding.then_some(now + 60))
                .chain(
                    crate::agents::process::wanted(&snapshot.settings)
                        .then_some(next_scan_at.max(now + 1)),
                )
                .min();
            Duration::from_secs(deadline.map_or(3600, |d| d.saturating_sub(now).clamp(1, 3600)))
        };
        let wait = errors
            .expires_in(now)
            .map_or(wait, |expiry| wait.min(expiry));
        let request = match receiver.recv_timeout(wait) {
            Ok(r) => Some(r),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(_) => break,
        };
        let elapsed = previous_time.elapsed();
        let wall_elapsed = SystemTime::now().duration_since(previous_wall);
        let interrupted = discontinuity(elapsed, wall_elapsed.ok(), wait, lifecycle.is_ok());
        previous_time = Instant::now();
        previous_wall = SystemTime::now();
        snapshot.engine.now = origin.elapsed().as_secs();
        if interrupted {
            snapshot
                .engine
                .reset_transient("Clock or system state changed · sessions cleared");
            previous_idle = None;
        }
        let mut reply = None;
        let mut mcp_reply = None;
        let mut quit = false;
        let mut snoozing = false;
        let mut open_dialog = false;
        let mut preview_countdown = false;
        let mut warning_failed = false;
        let mut show_menu = None;
        if let Some(request) = request {
            match request {
                Request::Mcp(call, tx) => {
                    let request_authorization = call.name == "doze.start_session";
                    let pending = snapshot.engine.agents.pending();
                    let result = crate::mcp::tools::call(
                        &mut snapshot.engine,
                        &snapshot.settings,
                        &snapshot.actions,
                        call,
                    );
                    // New approval requests appear in the panel and the menu bar icon; a
                    // notification points there instead of opening a window.
                    if request_authorization && snapshot.engine.agents.pending() > pending {
                        if let Some(session) = snapshot.engine.agents.items.iter().rev().find(|s| {
                            s.status == crate::mcp::sessions::Status::AwaitingAuthorization
                        }) {
                            let event = crate::core::sessions::Event::ApprovalNeeded {
                                agent: session.agent.label(),
                                project: session.project.clone(),
                            };
                            snapshot.engine.events.push(event);
                        }
                    }
                    mcp_reply = Some((tx, result));
                }
                Request::WarningFailed(error) => {
                    if snapshot.engine.countdown.is_some() {
                        warning_failed = true;
                        errors.report(
                            format!("Countdown window unavailable: {error}"),
                            snapshot.engine.now,
                        );
                    }
                }
                Request::AgentsUnavailable(error) => errors.agents = Some(error),
                Request::Lifecycle => {
                    snapshot
                        .engine
                        .reset_transient("System suspended or resumed · sessions cleared");
                    audio = None;
                    previous_idle = None;
                    previous_activity_marker = None;
                }
                Request::Operation(op, tx) => {
                    quit = matches!(op, Operation::Quit);
                    snoozing = matches!(op, Operation::Snooze);
                    open_dialog = matches!(
                        op,
                        Operation::OpenDialog { .. }
                            | Operation::OpenPage { .. }
                            | Operation::OpenPanel { .. }
                            | Operation::ConnectAgent { .. }
                    );
                    preview_countdown = matches!(op, Operation::PreviewCountdown);
                    if let Operation::ShowMenu { name } = &op {
                        show_menu = Some(name.clone());
                    }
                    if matches!(op, Operation::OpenPanel { .. }) {
                        // The panel footer shows the battery; read it fresh.
                        next_battery_at = 0;
                    }
                    snapshot.result = None;
                    let previous_settings = snapshot.settings.clone();
                    let result = apply(op, &mut snapshot, &path);
                    match &result {
                        Err(error) => errors.report(error.clone(), snapshot.engine.now),
                        // A successful save replaces a settings file that failed to load.
                        Ok(()) if snapshot.settings != previous_settings => errors.settings = None,
                        Ok(()) => {}
                    }
                    reply = Some((tx, result));
                }
            }
        }
        if quit {
            let _ = lid.set(false);
            let result = power.set_awake(false, false);
            if let Some((tx, _)) = reply {
                let _ = tx.send(result.map(|_| snapshot.clone()));
            }
            app.exit(0);
            break;
        }
        let now = snapshot.engine.now;
        if now >= next_battery_at {
            next_battery_at = now + 60;
            if let Some((percent, on_battery)) = platform::battery() {
                snapshot.engine.battery(
                    percent,
                    on_battery,
                    snapshot.settings.battery_floor_percent,
                );
            }
        }
        let scanning = crate::agents::process::wanted(&snapshot.settings);
        let detected = snapshot
            .engine
            .agents
            .items
            .iter()
            .any(|s| s.source == crate::agents::SessionSource::Process && !s.status.terminal());
        if (scanning && now >= next_scan_at) || (!scanning && detected) {
            next_scan_at = now + 30;
            let names = if scanning {
                platform::processes()
            } else {
                Vec::new()
            };
            let running = crate::agents::process::detect(&names);
            crate::agents::process::observe(&mut snapshot.engine, &snapshot.settings, &running);
        }
        let mut observation = None;
        let mut idle_seconds = None;
        let mut activity_marker = None;
        if !snapshot.engine.needs_audio() && snapshot.engine.completion_waiting(&snapshot.settings)
        {
            match idle.observe() {
                Ok(value) => idle_seconds = Some(value.seconds),
                // Without an idle reading the warning starts at once; it is cancellable.
                Err(_) => idle_seconds = None,
            }
        }
        if snapshot.engine.needs_audio() {
            if audio.is_none() && now >= retry_audio_at {
                match platform::NativeAudio::new() {
                    Ok(monitor) => audio = Some(monitor),
                    Err(e) => {
                        errors.observation = Some(format!("Audio unavailable: {e}"));
                        retry_audio_at = now + 30;
                    }
                }
            }
            let mut failure = None;
            if let Some(monitor) = audio.as_mut() {
                match monitor.sample() {
                    Ok(value) => observation = Some(value),
                    Err(e) => failure = Some(e),
                }
            }
            match idle.observe() {
                Ok(value) => {
                    idle_seconds = Some(value.seconds);
                    activity_marker = value.activity_marker;
                }
                Err(e) => failure = failure.or(Some(e)),
            }
            // While the monitor waits to retry, its creation failure remains current.
            if audio.is_some() || failure.is_some() {
                errors.observation = failure;
            }
        } else {
            audio = None;
            errors.observation = None;
        }
        let input_changed = match previous_activity_marker.zip(activity_marker) {
            Some((old, new)) => old != new,
            None => previous_idle
                .zip(idle_seconds)
                .is_some_and(|(old, new)| new < old),
        };
        // The Snooze click is an explicit delay request; subsequent input still cancels playback.
        let user_active = input_changed && !snoozing;
        previous_idle = idle_seconds;
        previous_activity_marker = activity_marker;
        let mut action = snapshot.engine.tick(
            snapshot.engine.now,
            observation,
            idle_seconds,
            user_active,
            &snapshot.settings,
        );
        if let Err(e) = power.set_awake(
            snapshot.engine.should_hold_awake(),
            snapshot.settings.allow_display_sleep,
        ) {
            action = None;
            if let Some((_, result)) = mcp_reply.as_mut() {
                *result = Err(format!("Wake assertion failed: {e}"));
            }
            errors.report(e, snapshot.engine.now);
            snapshot
                .engine
                .reset_transient("Power request failed · sessions cleared");
        }
        // A closed lid sleeps the PC whatever power requests say, so Windows also changes the
        // lid-close action, only while Doze keeps the PC awake.
        let hold_lid = snapshot.lid_supported
            && snapshot.settings.lid_closed_keep_awake
            && snapshot.engine.should_hold_awake();
        if let Err(error) = lid.set(hold_lid) {
            errors.report(error, snapshot.engine.now);
        }
        snapshot.assertions = power.describe();
        snapshot.assertions.extend(lid.describe());
        for event in std::mem::take(&mut snapshot.engine.events) {
            notify(&notifications, &snapshot.settings, event);
        }
        if let Some(c) = snapshot.engine.countdown.clone() {
            let key = Some((c.deadline, c.source));
            if let Some(only_warning) = announcement(
                key,
                attempted_notification,
                shown_notification,
                snapshot.settings.notifications,
                warning.is_err() || warning_failed,
            ) {
                attempted_notification = key;
                match notifications
                    .countdown(c.action, c.deadline.saturating_sub(snapshot.engine.now))
                {
                    Ok(()) => shown_notification = key,
                    Err(e) if only_warning => {
                        snapshot.engine.cancel_countdown();
                        errors.report(
                            format!("Countdown cancelled: no warning window or notification: {e}"),
                            snapshot.engine.now,
                        );
                    }
                    Err(e) => errors.report(
                        format!("Notification unavailable: {e}"),
                        snapshot.engine.now,
                    ),
                }
            }
        }
        if let Ok(warning) = &warning {
            warning.update(
                snapshot.engine.countdown.as_ref(),
                snapshot.engine.now,
                snoozing,
                &snapshot.settings,
            );
            if preview_countdown {
                warning.preview(snapshot.selected_action, &snapshot.settings);
            }
        }
        if let Some(action) = action {
            match power.execute(action) {
                Ok(()) => {
                    if let Some(notice) = power.take_notice() {
                        snapshot.engine.message = Some(notice);
                    }
                }
                Err(e) => {
                    errors.report(e, snapshot.engine.now);
                    snapshot.engine.message = Some("Power action failed".into());
                }
            }
        }
        snapshot.error = errors.current(snapshot.engine.now, snapshot.settings.agents.enabled);
        let tray_app = app.clone();
        let tray_snapshot = snapshot.clone();
        let _ = app.run_on_main_thread(move || crate::tray::update(&tray_app, &tray_snapshot));
        if let Some(name) = show_menu {
            let menu_app = app.clone();
            let _ = app.run_on_main_thread(move || crate::tray::show_menu(&menu_app, &name));
        }
        // Open windows follow the engine through this stream instead of polling it.
        platform::publish(&snapshot);
        if open_dialog {
            let dialog_snapshot = snapshot.clone();
            let dialog_sender = sender.clone();
            let _ = app.run_on_main_thread(move || {
                if let Err(error) = platform::dialogs::show(dialog_snapshot, dialog_sender) {
                    eprintln!("Could not open native dialog: {error}");
                }
            });
        }
        if snapshot.settings.logging && snapshot.error != logged_error {
            use std::io::Write;
            if let Some(error) = &snapshot.error {
                let log_path = path.with_extension("log");
                if std::fs::metadata(&log_path).is_ok_and(|m| m.len() > 256 * 1024) {
                    let _ = std::fs::remove_file(&log_path);
                }
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                {
                    let _ = writeln!(file, "{now}: {error}");
                }
            }
        }
        logged_error.clone_from(&snapshot.error);

        if let Some((tx, result)) = mcp_reply {
            let _ = tx.send(result);
        }
        if let Some((tx, result)) = reply {
            let _ = tx.send(result.map(|_| snapshot.clone()));
        }
        snapshot.result = None;
    }
}

/// Notifications for engine events, each behind its own setting.
fn notify(
    notifications: &platform::NativeNotifications,
    settings: &crate::core::sessions::Settings,
    event: crate::core::sessions::Event,
) {
    use crate::core::sessions::Event;
    let device = if cfg!(target_os = "macos") {
        "Mac"
    } else {
        "PC"
    };
    let (enabled, title, body) = match event {
        Event::ApprovalNeeded { agent, project } => (
            settings.notify_agent_approval,
            format!("{agent} wants to keep your {device} awake"),
            match project {
                Some(project) => format!("{project} · Allow or deny it in Doze."),
                None => "Allow or deny it in Doze.".into(),
            },
        ),
        Event::AgentsFinished { action } => (
            settings.notify_agents_finished,
            "All agents finished".into(),
            format!(
                "{} after the final warning. Open Doze to cancel or snooze.",
                action.label()
            ),
        ),
        Event::AgentStalled { agent } => (
            settings.notify_agent_stalled,
            format!("{agent} stopped checking in"),
            format!("Doze kept your {device} awake for 30 minutes, then let go without acting."),
        ),
        Event::KeepAwakeEnded => (
            settings.notify_keep_awake_ended,
            "Keep Awake ended".into(),
            "Normal sleep settings apply again.".into(),
        ),
        Event::BatteryGuard { percent } => (
            true,
            format!("Battery at {percent}%"),
            format!("Doze stopped keeping your {device} awake. It resumes when you plug in."),
        ),
    };
    if enabled {
        if let Err(error) = notifications.notify(&title, &body) {
            eprintln!("Notification unavailable: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_changes_only_signal_suspend_without_native_notifications() {
        let (minute, wait) = (Duration::from_secs(60), Duration::from_secs(3600));
        let hour_jump = Some(Duration::from_secs(3660));
        assert!(!discontinuity(minute, hour_jump, wait, true));
        assert!(!discontinuity(minute, None, wait, true));
        assert!(discontinuity(minute, hour_jump, wait, false));
        assert!(discontinuity(minute, None, wait, false));
        assert!(!discontinuity(minute, Some(minute), wait, false));
    }

    #[test]
    fn a_stalled_worker_always_signals_a_discontinuity() {
        let wait = Duration::from_secs(1);
        let stalled = Duration::from_secs(60);
        assert!(discontinuity(stalled, Some(stalled), wait, true));
        assert!(!discontinuity(
            Duration::from_secs(5),
            Some(Duration::from_secs(5)),
            wait,
            true
        ));
    }

    #[test]
    fn recent_errors_expire_and_restore_the_underlying_condition() {
        let mut errors = Errors {
            startup: Some("Countdown window unavailable".into()),
            ..Errors::default()
        };
        errors.report("Session already ended.".into(), 10);
        assert_eq!(
            errors.current(10, false).as_deref(),
            Some("Session already ended.")
        );
        assert_eq!(errors.expires_in(10), Some(Duration::from_secs(300)));
        assert_eq!(
            errors.current(309, false).as_deref(),
            Some("Session already ended.")
        );
        assert_eq!(
            errors.current(310, false).as_deref(),
            Some("Countdown window unavailable")
        );
        assert_eq!(errors.expires_in(310), None);
    }

    #[test]
    fn clearing_an_observation_failure_keeps_other_errors() {
        let mut errors = Errors {
            settings: Some("Could not read settings".into()),
            ..Errors::default()
        };
        errors.observation = Some("No active audio output device.".into());
        errors.report("Power action failed".into(), 0);
        errors.observation = None;
        assert_eq!(
            errors.current(1, false).as_deref(),
            Some("Power action failed")
        );
        assert_eq!(
            errors.current(300, false).as_deref(),
            Some("Could not read settings")
        );
        errors.settings = None;
        assert_eq!(errors.current(301, false), None);
    }

    #[test]
    fn countdowns_without_a_window_fall_back_to_one_required_notification() {
        let countdown = Some((300, Source::Timer));
        // Optional notifications follow the preference and are attempted once.
        assert_eq!(
            announcement(countdown, None, None, true, false),
            Some(false)
        );
        assert_eq!(announcement(countdown, countdown, None, true, false), None);
        assert_eq!(announcement(countdown, None, None, false, false), None);
        // Without the window, a notification is required whatever the preference.
        assert_eq!(announcement(countdown, None, None, false, true), Some(true));
        assert_eq!(
            announcement(countdown, countdown, countdown, false, true),
            None
        );
        // An optional attempt that failed is retried once the window is gone.
        assert_eq!(
            announcement(countdown, countdown, None, true, true),
            Some(true)
        );
        // A snoozed countdown is a new countdown.
        let snoozed = Some((1200, Source::Timer));
        assert_eq!(
            announcement(snoozed, countdown, countdown, false, true),
            Some(true)
        );
        assert_eq!(announcement(None, None, None, true, true), None);
    }

    #[test]
    fn agent_bridge_failure_only_shows_when_mcp_is_enabled() {
        let mut errors = Errors {
            agents: Some("Agent connections unavailable".into()),
            ..Errors::default()
        };
        assert_eq!(errors.current(0, false), None);
        assert_eq!(
            errors.current(0, true).as_deref(),
            Some("Agent connections unavailable")
        );
    }
}

use super::{
    model::{Operation, Request, Snapshot},
    operations::apply,
};
use crate::platform::{self, AudioMonitor, IdleMonitor, NotificationManager, PowerManager};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant, SystemTime},
};

pub(super) fn worker(
    app: tauri::AppHandle,
    path: PathBuf,
    mut snapshot: Snapshot,
    receiver: Receiver<Request>,
    sender: Sender<Request>,
) {
    let mut power = platform::NativePower::new();
    let idle = platform::NativeIdle;
    let notifications = platform::NativeNotifications(app.clone());
    let warning = platform::countdown::Warning::new(sender.clone());
    if let Err(error) = &warning {
        snapshot.error = Some(format!("Countdown window unavailable: {error}"));
    }
    let lifecycle = platform::lifecycle::Registration::new(sender.clone());
    if let Err(error) = &lifecycle {
        snapshot.error = Some(format!("Suspend notifications unavailable: {error}"));
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
    loop {
        let now = origin.elapsed().as_secs();
        snapshot.engine.now = now;
        let wait = if snapshot.engine.needs_audio() || snapshot.engine.countdown.is_some() {
            Duration::from_secs(1)
        } else if let Some(deadline) = snapshot
            .engine
            .awake_deadline
            .into_iter()
            .chain(snapshot.engine.timer.as_ref().map(|t| t.deadline))
            .min()
        {
            Duration::from_secs(deadline.saturating_sub(now).max(1))
        } else {
            Duration::from_secs(3600)
        };
        let request = match receiver.recv_timeout(wait) {
            Ok(r) => Some(r),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(_) => break,
        };
        let elapsed = previous_time.elapsed();
        let wall_elapsed = SystemTime::now().duration_since(previous_wall);
        let clock_changed =
            wall_elapsed.map_or(true, |w| w.abs_diff(elapsed) > Duration::from_secs(5));
        let overslept = elapsed > wait + Duration::from_secs(10);
        previous_time = Instant::now();
        previous_wall = SystemTime::now();
        snapshot.engine.now = origin.elapsed().as_secs();
        if clock_changed || overslept {
            snapshot
                .engine
                .reset_transient("Clock or system state changed · sessions cleared");
            previous_idle = None;
        }
        let mut reply = None;
        let mut quit = false;
        let mut snoozing = false;
        let mut open_dialog = false;
        let mut preview_countdown = false;
        if let Some(request) = request {
            match request {
                Request::WarningFailed(error) => {
                    if snapshot.engine.countdown.is_some() {
                        snapshot.engine.cancel_countdown();
                        snapshot.error = Some(format!("Countdown cancelled: {error}"));
                    }
                }
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
                    open_dialog = matches!(op, Operation::OpenDialog { .. });
                    preview_countdown = matches!(op, Operation::PreviewCountdown);
                    let result = apply(op, &mut snapshot, &path);
                    if let Err(error) = &result {
                        snapshot.error = Some(error.clone());
                    }
                    reply = Some((tx, result));
                }
            }
        }
        if quit {
            let result = power.set_awake(false, false);
            if let Some((tx, _)) = reply {
                let _ = tx.send(result.map(|_| snapshot.clone()));
            }
            app.exit(0);
            break;
        }
        let mut observation = None;
        let mut idle_seconds = None;
        let mut activity_marker = None;
        if snapshot.engine.needs_audio() {
            if audio.is_none() && now >= retry_audio_at {
                match platform::NativeAudio::new() {
                    Ok(monitor) => audio = Some(monitor),
                    Err(e) => {
                        snapshot.error = Some(format!("Audio unavailable: {e}"));
                        retry_audio_at = now + 30;
                    }
                }
            }
            if let Some(monitor) = audio.as_mut() {
                match monitor.sample() {
                    Ok(value) => {
                        observation = Some(value);
                        snapshot.error = None;
                    }
                    Err(e) => snapshot.error = Some(e),
                }
            }
            match idle.observe() {
                Ok(value) => {
                    idle_seconds = Some(value.seconds);
                    activity_marker = value.activity_marker;
                }
                Err(e) => snapshot.error = Some(e),
            }
        } else {
            audio = None;
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
        let old_countdown = snapshot
            .engine
            .countdown
            .as_ref()
            .map(|c| (c.deadline, c.source));
        let action = snapshot.engine.tick(
            snapshot.engine.now,
            observation,
            idle_seconds,
            user_active,
            &snapshot.settings,
        );
        if let Err(error) = &warning {
            if snapshot.engine.countdown.is_some() {
                snapshot.engine.cancel_countdown();
                snapshot.error = Some(format!("Countdown cancelled: {error}"));
            }
        }
        if let Err(e) = power.set_awake(
            snapshot.engine.should_hold_awake(),
            snapshot.settings.allow_display_sleep,
        ) {
            snapshot.error = Some(e);
            snapshot
                .engine
                .reset_transient("Power request failed · sessions cleared");
        }
        if let Some(c) = &snapshot.engine.countdown {
            if old_countdown != Some((c.deadline, c.source)) && snapshot.settings.notifications {
                if let Err(e) = notifications
                    .countdown(c.action, c.deadline.saturating_sub(snapshot.engine.now))
                {
                    snapshot.error = Some(format!("Notification unavailable: {e}"));
                }
            }
        }
        if let Ok(warning) = &warning {
            warning.update(
                snapshot.engine.countdown.as_ref(),
                snapshot.engine.now,
                snoozing,
                snapshot.settings.theme,
            );
            if preview_countdown {
                warning.preview(snapshot.selected_action, snapshot.settings.theme);
            }
        }
        if let Some(action) = action {
            if let Err(e) = power.execute(action) {
                snapshot.error = Some(e);
                snapshot.engine.message = Some("Power action failed".into());
            }
        }
        let tray_app = app.clone();
        let tray_snapshot = snapshot.clone();
        let _ = app.run_on_main_thread(move || crate::tray::update(&tray_app, &tray_snapshot));
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

        if let Some((tx, result)) = reply {
            let _ = tx.send(result.map(|_| snapshot.clone()));
        }
    }
}

use crate::{
    core::sessions::PowerAction,
    menu_icons::{self, Glyph},
    state::{AppState, DialogView, Operation, Request, Snapshot},
};
use tauri::{
    menu::{CheckMenuItem, IconMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager,
};

struct NativeMenu {
    agents: Submenu<tauri::Wry>,
    agent_signature: std::sync::Mutex<String>,
    quick: crate::quick_settings::QuickSettings,
    default_awake: MenuItem<tauri::Wry>,
    default_timer: MenuItem<tauri::Wry>,
    action_menu: Submenu<tauri::Wry>,
    status: MenuItem<tauri::Wry>,
    timer: MenuItem<tauri::Wry>,
    stop_awake: IconMenuItem<tauri::Wry>,
    extend: IconMenuItem<tauri::Wry>,
    stop_timer: IconMenuItem<tauri::Wry>,
    cancel: IconMenuItem<tauri::Wry>,
    snooze: IconMenuItem<tauri::Wry>,
    audio: CheckMenuItem<tauri::Wry>,
    playback: CheckMenuItem<tauri::Wry>,
    actions: Vec<(PowerAction, CheckMenuItem<tauri::Wry>)>,
}

pub(crate) fn dispatch(app: &tauri::AppHandle, operation: Operation) {
    if let Some(state) = app.try_state::<AppState>() {
        let (reply, _) = std::sync::mpsc::channel();
        let _ = state.sender.send(Request::Operation(operation, reply));
    }
}
pub(crate) fn show(app: &tauri::AppHandle) {
    dispatch(
        app,
        Operation::OpenDialog {
            view: DialogView::Settings,
        },
    );
}

fn item(app: &tauri::App, id: &str, text: &str) -> tauri::Result<MenuItem<tauri::Wry>> {
    MenuItem::with_id(app, id, text, true, None::<&str>)
}

pub(crate) fn setup(app: &tauri::App) -> tauri::Result<()> {
    let status = item(app, "status", "Normal sleep allowed")?;
    let timer = item(app, "timer_status", "No power timer")?;
    let awake = menu_icons::submenu(app, "awake_menu", "Keep Awake", Glyph::Awake)?;
    let sleep = menu_icons::submenu(app, "timer_menu", "Power Timer", Glyph::Timer)?;
    let default_awake = item(app, "awake_default", "Default (30 minutes)")?;
    let default_timer = item(app, "timer_default", "Default (30 minutes)")?;
    awake.append(&default_awake)?;
    sleep.append(&default_timer)?;
    for (seconds, label) in [
        (900, "15 minutes"),
        (1800, "30 minutes"),
        (3600, "1 hour"),
        (7200, "2 hours"),
    ] {
        awake.append(&item(app, &format!("awake:{seconds}"), label)?)?;
        sleep.append(&item(app, &format!("timer:{seconds}"), label)?)?;
    }
    awake.append(&item(app, "awake:forever", "Indefinitely")?)?;
    awake.append(&item(app, "awake:custom", "Custom duration…")?)?;
    awake.append(&item(app, "awake:time", "Until a specific time…")?)?;
    sleep.append(&item(app, "timer:custom", "Custom duration…")?)?;
    sleep.append(&item(app, "timer:time", "At a specific time…")?)?;
    let action_menu = Submenu::new(app, "Timer action", true)?;
    let mut actions = Vec::new();
    for (index, action) in [
        PowerAction::Sleep,
        PowerAction::Hibernate,
        PowerAction::Shutdown,
        PowerAction::Lock,
        PowerAction::DisplayOff,
    ]
    .into_iter()
    .enumerate()
    {
        let check = CheckMenuItem::with_id(
            app,
            format!("action:{index}"),
            action.label(),
            true,
            index == 0,
            None::<&str>,
        )?;
        action_menu.append(&check)?;
        actions.push((action, check));
    }
    let audio = CheckMenuItem::with_id(
        app,
        "audio",
        "Keep awake while audio plays",
        crate::platform::audio_supported(),
        false,
        None::<&str>,
    )?;
    let playback = CheckMenuItem::with_id(
        app,
        "playback",
        "Sleep after playback stops",
        crate::platform::audio_supported(),
        false,
        None::<&str>,
    )?;
    let stop_awake = menu_icons::item(app, "stop_awake", "No awake session to stop", Glyph::Stop)?;
    let extend = menu_icons::item(app, "extend", "No timed session to extend", Glyph::Add)?;
    let stop_timer = menu_icons::item(app, "stop_timer", "No timer to stop", Glyph::Stop)?;
    let cancel = menu_icons::item(app, "cancel", "No countdown to cancel", Glyph::Stop)?;
    let snooze = menu_icons::item(app, "snooze", "No countdown to snooze", Glyph::Timer)?;
    for inactive in [&stop_awake, &extend, &stop_timer, &cancel, &snooze] {
        inactive.set_enabled(false)?;
    }
    let settings = menu_icons::item(app, "settings", "Settings…", Glyph::Settings)?;
    let about = menu_icons::item(app, "about", "About Doze…", Glyph::Info)?;
    let quick = crate::quick_settings::QuickSettings::new(app)?;
    let preview = menu_icons::item(app, "preview", "Preview Countdown…", Glyph::Preview)?;
    preview.set_enabled(true)?;
    let quit = menu_icons::item(app, "quit", "Quit Doze", Glyph::Quit)?;
    let help = menu_icons::item(app, "help", "Menu Guide…", Glyph::Help)?;
    let countdown = menu_icons::submenu(app, "countdown_menu", "Countdown", Glyph::Timer)?;
    countdown.append_items(&[
        &cancel,
        &snooze,
        &PredefinedMenuItem::separator(app)?,
        &preview,
    ])?;
    let support = menu_icons::submenu(
        app,
        "support_menu",
        if cfg!(windows) {
            "Help && About"
        } else {
            "Help & About"
        },
        Glyph::Help,
    )?;
    support.append_items(&[&help, &about])?;
    // Session management lives beside its start controls rather than filling the root with
    // inactive rows. Disabled commands still explain why they cannot run.
    awake.append_items(&[&PredefinedMenuItem::separator(app)?, &extend, &stop_awake])?;
    sleep.prepend_items(&[&action_menu, &PredefinedMenuItem::separator(app)?])?;
    sleep.append_items(&[&PredefinedMenuItem::separator(app)?, &stop_timer])?;
    let separators = (0..4)
        .map(|_| PredefinedMenuItem::separator(app))
        .collect::<tauri::Result<Vec<_>>>()?;
    let agents = Submenu::new(app, "Agents", true)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &timer,
            &separators[0],
            &awake,
            &audio,
            &separators[1],
            &sleep,
            &playback,
            &countdown,
            &separators[2],
            &agents,
            &quick.menu,
            &settings,
            &support,
            &separators[3],
            &quit,
        ],
    )?;
    app.manage(NativeMenu {
        agents,
        agent_signature: std::sync::Mutex::new(String::new()),
        quick,
        default_awake,
        default_timer,
        action_menu,
        status,
        timer,
        stop_awake,
        extend,
        stop_timer,
        cancel,
        snooze,
        audio,
        playback,
        actions,
    });
    TrayIconBuilder::with_id("doze")
        .icon(image(0))
        .tooltip("Doze · Normal sleep allowed")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .icon_as_template(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            if let Some(operation) = agent_operation(id) {
                dispatch(app, operation);
                return;
            }
            if let Some(operation) = crate::quick_settings::operation(id) {
                dispatch(app, operation);
                return;
            }
            let operation = match id {
                "about" => Operation::OpenDialog {
                    view: DialogView::About,
                },
                "awake_default" => Operation::KeepAwakeDefault,
                "timer_default" => Operation::ScheduleDefault,
                "help" | "status" | "timer_status" => Operation::OpenDialog {
                    view: DialogView::Help,
                },
                "preview" => Operation::PreviewCountdown,
                "settings" => Operation::OpenDialog {
                    view: DialogView::Settings,
                },
                "awake:custom" => Operation::OpenDialog {
                    view: DialogView::AwakeDuration,
                },
                "awake:time" => Operation::OpenDialog {
                    view: DialogView::AwakeTime,
                },
                "timer:custom" => Operation::OpenDialog {
                    view: DialogView::TimerDuration,
                },
                "timer:time" => Operation::OpenDialog {
                    view: DialogView::TimerTime,
                },
                "awake:forever" => Operation::KeepAwake { seconds: None },
                "stop_awake" => Operation::StopAwake,
                "extend" => Operation::ExtendAwake { seconds: 900 },
                "audio" => Operation::ToggleWhileAudio,
                "playback" => Operation::TogglePlayback,
                "stop_timer" => Operation::StopTimer,
                "cancel" => Operation::Cancel,
                "snooze" => Operation::Snooze,
                "quit" => Operation::Quit,
                _ => {
                    let Some((prefix, value)) = id.split_once(':') else {
                        return;
                    };
                    let Ok(value) = value.parse::<u64>() else {
                        return;
                    };
                    match prefix {
                        "awake" => Operation::KeepAwake {
                            seconds: Some(value),
                        },
                        "timer" => Operation::ScheduleSelected { seconds: value },
                        "action" => {
                            let menu = app.state::<NativeMenu>();
                            let Some((action, _)) = menu.actions.get(value as usize) else {
                                return;
                            };
                            Operation::SelectAction { action: *action }
                        }
                        _ => return,
                    }
                }
            };
            dispatch(app, operation);
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(event, TrayIconEvent::Click { .. }) {
                dispatch(tray.app_handle(), Operation::Refresh);
            }
        })
        .build(app)?;
    dispatch(app.handle(), Operation::Refresh);
    Ok(())
}

fn remaining(deadline: u64, now: u64) -> String {
    let seconds = deadline.saturating_sub(now);
    if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, seconds / 60 % 60)
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

/// Minutes left, rounded up. Awake and timer labels refresh once a minute; seconds
/// would look frozen between refreshes.
fn minutes_left(deadline: u64, now: u64) -> String {
    let minutes = deadline.saturating_sub(now).div_ceil(60);
    if minutes >= 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

pub(crate) fn status_text(snapshot: &Snapshot) -> String {
    let engine = &snapshot.engine;
    if let Some(error) = &snapshot.error {
        format!("Needs attention: {error}")
    } else if engine
        .agents
        .items
        .iter()
        .any(|s| s.status == crate::mcp::sessions::Status::ConnectionLost)
    {
        "Agent connection lost · keeping awake".into()
    } else if engine.agents.holds_awake() {
        "Keeping awake · agents working".into()
    } else if let Some(deadline) = engine.awake_deadline {
        format!(
            "Keeping awake · {} left",
            minutes_left(deadline, engine.now)
        )
    } else if engine.awake {
        "Keeping awake · indefinitely".into()
    } else if engine.countdown.is_some() {
        "Keeping awake · countdown running".into()
    } else if engine.timer.is_some() {
        "Keeping awake · timer running".into()
    } else if engine.should_hold_awake() {
        "Keeping awake · audio playback".into()
    } else {
        "Normal sleep allowed".into()
    }
}

/// Short text beside the menu bar icon: the final warning in seconds, otherwise the power
/// timer, otherwise a timed keep-awake session.
pub(crate) fn menu_bar_title(snapshot: &Snapshot) -> Option<String> {
    let engine = &snapshot.engine;
    if !snapshot.settings.menu_bar_time {
        return None;
    }
    if let Some(countdown) = &engine.countdown {
        let seconds = countdown.deadline.saturating_sub(engine.now);
        return Some(format!("{}:{:02}", seconds / 60, seconds % 60));
    }
    engine
        .timer
        .as_ref()
        .map(|timer| timer.deadline)
        .or(engine.awake_deadline)
        .map(|deadline| minutes_left(deadline, engine.now))
}

pub(crate) fn update(app: &tauri::AppHandle, snapshot: &Snapshot) {
    let Some(menu) = app.try_state::<NativeMenu>() else {
        return;
    };
    let engine = &snapshot.engine;
    let status = status_text(snapshot);
    menu.quick.update(&snapshot.settings);
    update_agents(app, &menu, snapshot);
    let _ = menu.default_awake.set_text(format!(
        "Default ({} minutes)",
        snapshot.settings.default_awake_minutes
    ));
    let _ = menu.default_timer.set_text(format!(
        "Default ({} minutes)",
        snapshot.settings.default_timer_minutes
    ));
    let timer = if let Some(countdown) = &engine.countdown {
        format!(
            "{} in {} — countdown",
            countdown.action.label(),
            remaining(countdown.deadline, engine.now)
        )
    } else if let Some(timer) = &engine.timer {
        format!(
            "{} in {}",
            timer.action.label(),
            minutes_left(timer.deadline, engine.now)
        )
    } else if engine.playback_enabled {
        use crate::core::sessions::Phase;
        let phase = match engine.playback_phase {
            Phase::Idle | Phase::Cancelled | Phase::Completed => "waiting for playback",
            Phase::Active => "playing",
            Phase::GracePeriod => "waiting for silence and idle",
            Phase::Countdown => "countdown",
        };
        format!("After Playback: {phase}")
    } else {
        "No power action scheduled".into()
    };
    let _ = menu.status.set_text(&status);
    let _ = menu.timer.set_text(&timer);
    let _ = menu.action_menu.set_text(format!(
        "Timer action: {}",
        snapshot.selected_action.label()
    ));
    let _ = menu.playback.set_text(format!(
        "{} after playback stops",
        snapshot.settings.playback_action.label()
    ));
    let _ = menu
        .stop_awake
        .set_enabled(engine.awake || engine.while_audio);
    let _ = menu.extend.set_enabled(engine.awake_deadline.is_some());
    let _ = menu.stop_timer.set_enabled(
        engine.timer.is_some()
            || engine
                .countdown
                .as_ref()
                .is_some_and(|c| c.source == crate::core::countdown::Source::Timer),
    );
    let _ = menu.cancel.set_enabled(engine.countdown.is_some());
    let _ = menu.snooze.set_enabled(engine.countdown.is_some());
    let _ = menu
        .stop_awake
        .set_text(if engine.awake || engine.while_audio {
            "Stop keeping awake"
        } else {
            "No awake session to stop"
        });
    let _ = menu.extend.set_text(if engine.awake_deadline.is_some() {
        "Extend awake by 15 minutes"
    } else {
        "No timed awake session to extend"
    });
    let has_timer = engine.timer.is_some()
        || engine
            .countdown
            .as_ref()
            .is_some_and(|c| c.source == crate::core::countdown::Source::Timer);
    let _ = menu.stop_timer.set_text(if has_timer {
        "Stop timer"
    } else {
        "No timer to stop"
    });
    let _ = menu.cancel.set_text(if engine.countdown.is_some() {
        "Cancel countdown"
    } else {
        "No countdown to cancel"
    });
    let _ = menu.snooze.set_text(if engine.countdown.is_some() {
        "Snooze 15 minutes"
    } else {
        "No countdown to snooze"
    });
    let _ = menu.audio.set_checked(engine.while_audio);
    let _ = menu.playback.set_checked(engine.playback_enabled);
    for (action, check) in &menu.actions {
        let _ = check.set_enabled(snapshot.actions.contains(action));
        let _ = check.set_checked(*action == snapshot.selected_action);
    }
    if let Some(tray) = app.tray_by_id("doze") {
        let state = if engine.countdown.is_some() {
            2
        } else {
            u8::from(engine.should_hold_awake())
        };
        let _ = tray.set_tooltip(Some(&format!("Doze · {status}\n{timer}")));
        let _ = tray.set_icon_with_as_template(Some(image(state)), cfg!(target_os = "macos"));
        #[cfg(target_os = "macos")]
        let _ = tray.set_title(menu_bar_title(snapshot));
    }
}
// Native template images ignore color, so countdown uses a ring and awake a filled dot.
pub(crate) fn image(status: u8) -> tauri::image::Image<'static> {
    let mut pixels = vec![0u8; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let mut moon_samples = 0;
            let mut badge_samples = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = x as f64 + (sx as f64 + 0.5) / 4.0;
                    let py = y as f64 + (sy as f64 + 0.5) / 4.0;
                    let moon =
                        (px - 14.0).hypot(py - 15.0) < 11.0 && (px - 19.0).hypot(py - 10.0) > 10.0;
                    let radius = (px - 25.0).hypot(py - 25.0);
                    let badge = status > 0 && radius < 4.0 && (status != 2 || radius > 2.2);
                    if badge {
                        badge_samples += 1;
                    } else if moon {
                        moon_samples += 1;
                    }
                }
            }
            let coverage = moon_samples + badge_samples;
            if coverage > 0 {
                let i = (y * 32 + x) * 4;
                let color = if badge_samples > moon_samples {
                    if status == 2 {
                        [238, 174, 74]
                    } else {
                        [155, 139, 239]
                    }
                } else {
                    [165, 166, 180]
                };
                pixels[i..i + 3].copy_from_slice(&color);
                pixels[i + 3] = (coverage * 255 / 16) as u8;
            }
        }
    }
    tauri::image::Image::new_owned(pixels, 32, 32)
}

fn agent_operation(id: &str) -> Option<Operation> {
    let parts: Vec<_> = id.split(':').collect();
    Some(match parts.as_slice() {
        ["agents", "settings"] => Operation::OpenDialog {
            view: DialogView::Agents,
        },
        ["agents", "enable"] => Operation::AgentEnabled,
        ["agents", "connect", name] => Operation::ConnectAgent {
            name: (*name).into(),
        },
        ["agents", "lease", seconds] => Operation::AgentLease {
            seconds: seconds.parse().ok()?,
        },
        ["agents", "once" | "deny", id] => Operation::AuthorizeAgent {
            id: (*id).into(),
            decision: parts[1].into(),
        },
        ["agents", "cancel", id] => Operation::CancelAgent { id: (*id).into() },
        ["agents", "wait", id] => Operation::WaitAgent { id: (*id).into() },
        ["agents", "finish", id] => Operation::FinishAgent { id: (*id).into() },
        ["agents", "revoke", id] => Operation::RevokeAgent { id: (*id).into() },
        _ => return None,
    })
}
fn update_agents(app: &tauri::AppHandle, menu: &NativeMenu, snapshot: &Snapshot) {
    use crate::mcp::sessions::Status;
    let signature = format!(
        "{:?}{:?}{}",
        snapshot.settings.agents,
        snapshot.engine.agents.items,
        snapshot.engine.now / 60
    );
    let Ok(mut previous) = menu.agent_signature.lock() else {
        return;
    };
    if *previous == signature {
        return;
    }
    *previous = signature;
    let update = || -> tauri::Result<()> {
        for item in menu.agents.items()? {
            menu.agents.remove(&item)?;
        }
        menu.agents.append(&CheckMenuItem::with_id(
            app,
            "agents:enable",
            "Enable MCP",
            true,
            snapshot.settings.agents.enabled,
            None::<&str>,
        )?)?;
        menu.agents.append(&MenuItem::with_id(
            app,
            "agents:settings",
            "Agent settings and connections…",
            true,
            None::<&str>,
        )?)?;
        for session in snapshot
            .engine
            .agents
            .items
            .iter()
            .filter(|s| !s.status.terminal())
        {
            let text = format!(
                "{} · {} · {}m · {}",
                session.client_name,
                session.reason,
                snapshot.engine.now.saturating_sub(session.created_at) / 60,
                if session.status == Status::AwaitingAuthorization {
                    "Approval needed"
                } else if session.status == Status::ConnectionLost {
                    "Connection lost · keeping awake"
                } else {
                    "Working"
                }
            );
            let row = Submenu::new(app, text.replace('&', "&&"), true)?;
            let action = session
                .completion_action
                .map_or("Return to normal", PowerAction::label);
            row.append(&MenuItem::new(
                app,
                format!("When finished: {action}"),
                false,
                None::<&str>,
            )?)?;
            if session.status == Status::AwaitingAuthorization {
                for (id, label) in [("once", "Allow Once"), ("deny", "Deny")] {
                    row.append(&MenuItem::with_id(
                        app,
                        format!("agents:{id}:{}", session.session_id),
                        label,
                        true,
                        None::<&str>,
                    )?)?;
                }
            } else {
                row.append(&MenuItem::with_id(
                    app,
                    format!("agents:cancel:{}", session.session_id),
                    "Cancel session",
                    true,
                    None::<&str>,
                )?)?;
                if session.status == Status::ConnectionLost {
                    row.append(&MenuItem::new(
                        app,
                        format!(
                            "Last heartbeat {}m ago",
                            snapshot.engine.now.saturating_sub(session.last_heartbeat) / 60
                        ),
                        false,
                        None::<&str>,
                    )?)?;
                    row.append(&MenuItem::with_id(
                        app,
                        format!("agents:wait:{}", session.session_id),
                        "Wait 30 minutes",
                        true,
                        None::<&str>,
                    )?)?;
                    row.append(&MenuItem::with_id(
                        app,
                        format!("agents:finish:{}", session.session_id),
                        "End and apply completion action",
                        true,
                        None::<&str>,
                    )?)?;
                }
            }
            menu.agents.append(&row)?;
        }
        Ok(())
    };
    if let Err(error) = update() {
        eprintln!("Could not update agent menu: {error}");
        previous.clear();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn session_labels_round_up_to_whole_minutes() {
        assert_eq!(super::minutes_left(100, 100), "0m");
        assert_eq!(super::minutes_left(130, 100), "1m");
        assert_eq!(super::minutes_left(1900, 100), "30m");
        assert_eq!(super::minutes_left(3700, 100), "1h 0m");
        assert_eq!(super::minutes_left(7301, 100), "2h 1m");
    }
    #[test]
    fn menu_bar_title_prefers_the_warning_then_the_timer_then_awake() {
        use crate::core::{
            countdown::{Countdown, Source},
            sessions::{Engine, PowerAction, Settings, Timer},
        };
        use crate::state::{DialogView, Snapshot};
        let mut snapshot = Snapshot {
            settings_path: "settings.json".into(),
            engine: Engine::default(),
            settings: Settings::default(),
            actions: vec![PowerAction::Sleep],
            audio_supported: true,
            startup_supported: true,
            error: None,
            selected_action: PowerAction::Sleep,
            view: DialogView::Settings,
        };
        assert_eq!(super::menu_bar_title(&snapshot), None);
        snapshot.engine.now = 100;
        snapshot.engine.awake_deadline = Some(100 + 2520);
        assert_eq!(super::menu_bar_title(&snapshot).as_deref(), Some("42m"));
        snapshot.engine.timer = Some(Timer {
            deadline: 100 + 3900,
            action: PowerAction::Sleep,
        });
        assert_eq!(super::menu_bar_title(&snapshot).as_deref(), Some("1h 5m"));
        snapshot.engine.countdown = Some(Countdown {
            deadline: 100 + 287,
            action: PowerAction::Sleep,
            source: Source::Timer,
        });
        assert_eq!(super::menu_bar_title(&snapshot).as_deref(), Some("4:47"));
        snapshot.settings.menu_bar_time = false;
        assert_eq!(super::menu_bar_title(&snapshot), None);
        // Older settings files gain the preference switched on.
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert!(old.menu_bar_time);
    }
    #[test]
    fn template_badges_distinguish_awake_from_countdown_without_color() {
        let center_alpha = (25 * 32 + 25) * 4 + 3;
        assert_eq!(super::image(1).rgba()[center_alpha], 255);
        assert_eq!(super::image(2).rgba()[center_alpha], 0);
    }
}

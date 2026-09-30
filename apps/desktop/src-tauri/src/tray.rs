use crate::{
    core::sessions::PowerAction,
    state::{AppState, DialogView, Operation, Request, Snapshot},
};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager,
};

struct NativeMenu {
    status: MenuItem<tauri::Wry>,
    timer: MenuItem<tauri::Wry>,
    stop_awake: MenuItem<tauri::Wry>,
    extend: MenuItem<tauri::Wry>,
    stop_timer: MenuItem<tauri::Wry>,
    cancel: MenuItem<tauri::Wry>,
    snooze: MenuItem<tauri::Wry>,
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
    let status = item(app, "status", "Ready to rest")?;
    status.set_enabled(false)?;
    let timer = item(app, "timer_status", "No sleep timer")?;
    timer.set_enabled(false)?;
    let awake = Submenu::new(app, "Keep awake", true)?;
    let sleep = Submenu::new(app, "Sleep timer", true)?;
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
        "After Playback",
        crate::platform::audio_supported(),
        false,
        None::<&str>,
    )?;
    let stop_awake = item(app, "stop_awake", "Stop keeping awake")?;
    let extend = item(app, "extend", "Extend awake by 15 minutes")?;
    let stop_timer = item(app, "stop_timer", "Stop sleep timer")?;
    let cancel = item(app, "cancel", "Cancel countdown")?;
    let snooze = item(app, "snooze", "Snooze 15 minutes")?;
    let settings = item(app, "settings", "Settings…")?;
    let preview = item(app, "preview", "Preview countdown…")?;
    let quit = item(app, "quit", "Quit Doze")?;
    let separators = (0..4)
        .map(|_| PredefinedMenuItem::separator(app))
        .collect::<tauri::Result<Vec<_>>>()?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &timer,
            &separators[0],
            &awake,
            &stop_awake,
            &extend,
            &audio,
            &separators[1],
            &action_menu,
            &sleep,
            &stop_timer,
            &playback,
            &separators[2],
            &cancel,
            &snooze,
            &separators[3],
            &settings,
            &preview,
            &quit,
        ],
    )?;
    app.manage(NativeMenu {
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
        .tooltip("Doze · Ready to rest")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            let operation = match id {
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
    format!(
        "{}h {}m {}s",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

pub(crate) fn update(app: &tauri::AppHandle, snapshot: &Snapshot) {
    let Some(menu) = app.try_state::<NativeMenu>() else {
        return;
    };
    let engine = &snapshot.engine;
    let status = if let Some(error) = &snapshot.error {
        format!("Doze: {error}")
    } else if let Some(deadline) = engine.awake_deadline {
        format!(
            "Keeping awake: {} remaining",
            remaining(deadline, engine.now)
        )
    } else if engine.awake {
        "Keeping awake indefinitely".into()
    } else if engine.while_audio {
        format!(
            "Audio awake: {}",
            if engine.audio_active {
                "playing"
            } else {
                "waiting"
            }
        )
    } else {
        "Ready to rest".into()
    };
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
            remaining(timer.deadline, engine.now)
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
        "No sleep timer".into()
    };
    let _ = menu.status.set_text(&status);
    let _ = menu.timer.set_text(&timer);
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
        let _ = tray.set_icon(Some(image(state)));
    }
}
// Crisp native tray glyph, with a violet awake dot or amber countdown dot.
pub(crate) fn image(status: u8) -> tauri::image::Image<'static> {
    let mut pixels = vec![0u8; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let moon = (x as f64 - 14.0).powi(2) + (y as f64 - 15.0).powi(2) < 11.0f64.powi(2)
                && (x as f64 - 19.0).powi(2) + (y as f64 - 10.0).powi(2) > 10.0f64.powi(2);
            let dot = status > 0
                && (x as f64 - 25.0).powi(2) + (y as f64 - 25.0).powi(2) < 4.0f64.powi(2);
            if moon || dot {
                let i = (y * 32 + x) * 4;
                pixels[i..i + 4].copy_from_slice(if dot {
                    if status == 2 {
                        &[238, 174, 74, 255]
                    } else {
                        &[155, 139, 239, 255]
                    }
                } else {
                    &[165, 166, 180, 255]
                });
            }
        }
    }
    tauri::image::Image::new_owned(pixels, 32, 32)
}

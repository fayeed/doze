use crate::{
    core::sessions::{IconClick, PowerAction},
    menu_icons::{self, Glyph},
    state::{AppState, DialogView, Operation, Request, Snapshot},
};
use tauri::{
    menu::{CheckMenuItem, IconMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
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
    /// The whole menu, for the clicks that open it.
    root: Menu<tauri::Wry>,
    /// What a plain click opens, from the latest settings.
    click: std::sync::Mutex<IconClick>,
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

/// ⌥-click opens whichever of panel and menu a plain click does not.
#[cfg(target_os = "macos")]
fn option_pressed() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceFlagsState(state: i32) -> u64;
    }
    const COMBINED_SESSION_STATE: i32 = 0;
    const ALTERNATE: u64 = 0x0008_0000;
    unsafe { CGEventSourceFlagsState(COMBINED_SESSION_STATE) & ALTERNATE != 0 }
}
#[cfg(not(target_os = "macos"))]
fn option_pressed() -> bool {
    false
}

/// Whether this click opens the panel; otherwise it opens the menu.
fn opens_panel(button: MouseButton, option: bool, setting: IconClick) -> bool {
    let primary = button == MouseButton::Left && !option;
    primary == (setting == IconClick::Panel)
}

fn on_click(
    tray: &tauri::tray::TrayIcon,
    button: MouseButton,
    rect: tauri::Rect,
    cursor: tauri::PhysicalPosition<f64>,
) {
    let app = tray.app_handle();
    let Some(menu) = app.try_state::<NativeMenu>() else {
        return;
    };
    let setting = menu.click.lock().map_or(IconClick::Panel, |c| *c);
    if opens_panel(button, option_pressed(), setting) {
        // macOS drops the panel under the status item. Windows docks the flyout at the right
        // of the taskbar like its own Quick Settings, so it only needs the clicked monitor,
        // and the cursor (on the icon at click time) always identifies it.
        let (position, size) = if cfg!(windows) {
            (cursor, tauri::PhysicalSize::new(0.0, 0.0))
        } else {
            (
                rect.position.to_physical::<f64>(1.0),
                rect.size.to_physical::<f64>(1.0),
            )
        };
        let scale = app
            .monitor_from_point(position.x, position.y)
            .ok()
            .flatten()
            .map(|monitor| monitor.scale_factor());
        dispatch(
            app,
            Operation::OpenPanel {
                anchor: serde_json::json!({
                    "x": position.x,
                    "y": position.y,
                    "width": size.width,
                    "height": size.height,
                    "scale": scale,
                }),
            },
        );
    } else {
        let _ = tray.set_menu(Some(menu.root.clone()));
        let _ = tray.with_inner_tray_icon(|inner| inner.show_menu());
    }
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
    // Menu titles treat a single & as a mnemonic marker on every platform, including macOS.
    let support = menu_icons::submenu(app, "support_menu", "Help && About", Glyph::Help)?;
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
        root: menu.clone(),
        click: std::sync::Mutex::new(IconClick::Panel),
    });
    let tray = TrayIconBuilder::with_id("doze")
        .icon(image(IconState::Normal, taskbar_ink()))
        .tooltip("Doze · Normal sleep allowed")
        .menu(&menu)
        .show_menu_on_left_click(false)
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
            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                rect,
                position,
                ..
            } = event
            {
                if matches!(button, MouseButton::Left | MouseButton::Right) {
                    on_click(tray, button, rect, position);
                }
            }
        })
        .build(app)?;
    // Doze decides which button opens the menu and which the panel.
    let _ = tray.with_inner_tray_icon(|inner| inner.set_show_menu_on_right_click(false));
    #[cfg(windows)]
    watch_taskbar_theme(app.handle().clone());
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
        let only_jobs = engine
            .agents
            .items
            .iter()
            .filter(|s| s.holds())
            .all(|s| s.client_id == crate::mcp::sessions::LOCAL_CLIENT_ID);
        if only_jobs {
            "Keeping awake · command running".into()
        } else {
            "Keeping awake · agents working".into()
        }
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

/// The first status line in the panel and the Settings sidebar.
pub(crate) fn status_short(snapshot: &Snapshot) -> String {
    let engine = &snapshot.engine;
    if let Some(countdown) = &engine.countdown {
        match countdown.source {
            crate::core::countdown::Source::Agents => "All agents finished".into(),
            crate::core::countdown::Source::Timer => "Power timer finished".into(),
            crate::core::countdown::Source::Playback => "Playback stopped".into(),
        }
    } else if engine.should_hold_awake() {
        "Keeping awake".into()
    } else {
        "Normal sleep allowed".into()
    }
}

/// The second status line: who or what holds the computer awake, or what is scheduled.
pub(crate) fn status_detail(snapshot: &Snapshot) -> String {
    let engine = &snapshot.engine;
    let working: Vec<String> = {
        let mut names: Vec<String> = engine
            .agents
            .items
            .iter()
            .filter(|s| s.holds())
            .map(|s| s.agent.label())
            .collect();
        names.dedup();
        names
    };
    if let Some(countdown) = &engine.countdown {
        format!(
            "{} in {}",
            countdown.action.label(),
            remaining(countdown.deadline, engine.now)
        )
    } else if engine.agents.holds_awake() {
        match working.as_slice() {
            [one] => format!("For {one}"),
            many => format!("For {} agents", many.len()),
        }
    } else if let Some(deadline) = engine.awake_deadline {
        format!("{} left", minutes_left(deadline, engine.now))
    } else if engine.awake {
        "Until you stop it".into()
    } else if let Some(timer) = &engine.timer {
        format!(
            "{} in {}",
            timer.action.label(),
            minutes_left(timer.deadline, engine.now)
        )
    } else if engine.should_hold_awake() {
        "While audio plays".into()
    } else if engine.playback_enabled {
        format!(
            "{} after playback stops",
            snapshot.settings.playback_action.label()
        )
    } else {
        "No power action scheduled".into()
    }
}

/// The four menu bar and tray glyphs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconState {
    /// Hollow sun: normal sleep allowed.
    Normal,
    /// Whole sun: keeping awake.
    Awake,
    /// Sun with a dot: an agent waits for approval.
    Attention,
    /// Banded sun: the final warning is counting down.
    Countdown,
}

pub(crate) fn icon_state_of(snapshot: &Snapshot) -> IconState {
    let engine = &snapshot.engine;
    if engine.countdown.is_some() {
        IconState::Countdown
    } else if engine.agents.pending() > 0 {
        IconState::Attention
    } else if engine.should_hold_awake() {
        IconState::Awake
    } else {
        IconState::Normal
    }
}

pub(crate) fn icon_state(snapshot: &Snapshot) -> &'static str {
    match icon_state_of(snapshot) {
        IconState::Normal => "normal",
        IconState::Awake => "awake",
        IconState::Attention => "attention",
        IconState::Countdown => "countdown",
    }
}

/// Short text beside the menu bar icon: the final warning in minutes and seconds, otherwise
/// the time left on the power timer or keep-awake, otherwise the number of working agents
/// when only agents hold the Mac awake.
// Windows tray icons have no title; the tooltip already carries the same information.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn menu_bar_title(snapshot: &Snapshot) -> Option<String> {
    let engine = &snapshot.engine;
    let settings = &snapshot.settings;
    if let Some(countdown) = &engine.countdown {
        if !settings.menu_bar_time {
            return None;
        }
        let seconds = countdown.deadline.saturating_sub(engine.now);
        return Some(format!("{}:{:02}", seconds / 60, seconds % 60));
    }
    let deadline = engine
        .timer
        .as_ref()
        .map(|timer| timer.deadline)
        .or(engine.awake_deadline);
    if let Some(deadline) = deadline {
        if !settings.menu_bar_time {
            return None;
        }
        let minutes = deadline.saturating_sub(engine.now).div_ceil(60);
        return Some(if minutes >= 60 {
            format!("{}:{:02}", minutes / 60, minutes % 60)
        } else {
            format!("{minutes}m")
        });
    }
    let working = engine.agents.working();
    let only_agents = !engine.awake && engine.timer.is_none();
    (settings.menu_bar_agent_count && only_agents && working > 0).then(|| working.to_string())
}

/// The taskbar's colour scheme decides the tray glyph's ink: white on a dark taskbar,
/// black on a light one. macOS template images ignore colour.
#[cfg(windows)]
fn taskbar_ink() -> [u8; 3] {
    use windows::{
        core::w,
        Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
    };
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let light = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    }
    .is_ok()
        && value == 1;
    if light {
        [0, 0, 0]
    } else {
        [255, 255, 255]
    }
}
#[cfg(not(windows))]
fn taskbar_ink() -> [u8; 3] {
    [0, 0, 0]
}

/// Swaps the tray glyph's ink when the taskbar switches between light and dark.
#[cfg(windows)]
fn watch_taskbar_theme(app: tauri::AppHandle) {
    use windows::{
        core::w,
        Win32::System::Registry::{
            RegCloseKey, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
            KEY_NOTIFY, REG_NOTIFY_CHANGE_LAST_SET,
        },
    };
    let _ = std::thread::Builder::new()
        .name("doze-taskbar-theme".into())
        .spawn(move || unsafe {
            let mut key = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                0,
                KEY_NOTIFY,
                &mut key,
            )
            .is_err()
            {
                return;
            }
            // Blocks until the key changes; no polling.
            while RegNotifyChangeKeyValue(key, false, REG_NOTIFY_CHANGE_LAST_SET, None, false)
                .is_ok()
            {
                dispatch(&app, Operation::Refresh);
            }
            let _ = RegCloseKey(key);
        });
}

pub(crate) fn update(app: &tauri::AppHandle, snapshot: &Snapshot) {
    let Some(menu) = app.try_state::<NativeMenu>() else {
        return;
    };
    if let Ok(mut click) = menu.click.lock() {
        *click = snapshot.settings.icon_click_opens;
    }
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
        format!("Snooze {} minutes", snapshot.settings.snooze_minutes)
    } else {
        "No countdown to snooze".into()
    });
    let _ = menu.audio.set_checked(engine.while_audio);
    let _ = menu.playback.set_checked(engine.playback_enabled);
    for (action, check) in &menu.actions {
        let _ = check.set_enabled(snapshot.actions.contains(action));
        let _ = check.set_checked(*action == snapshot.selected_action);
    }
    if let Some(tray) = app.tray_by_id("doze") {
        let pending = engine.agents.pending();
        // Windows shows the time left in the tooltip unless turned off in Settings › General.
        let timer_line = if cfg!(windows) && !snapshot.settings.menu_bar_time {
            String::new()
        } else {
            format!("\n{timer}")
        };
        let tooltip = if pending > 0 {
            format!("Doze · {status}\n{pending} agent request waiting for approval{timer_line}")
        } else {
            format!("Doze · {status}{timer_line}")
        };
        let _ = tray.set_tooltip(Some(&tooltip));
        let _ = tray.set_icon_with_as_template(
            Some(image(icon_state_of(snapshot), taskbar_ink())),
            cfg!(target_os = "macos"),
        );
        #[cfg(target_os = "macos")]
        let _ = tray.set_title(menu_bar_title(snapshot));
    }
}

/// The brand glyphs (icons/glyphs/doze-glyph-*.svg, a 16-unit grid) rasterized at 32 px:
/// a ring while normal sleep is allowed, a whole sun while awake, the sun with a dot when an
/// agent waits for approval, and the banded setting sun during the final warning.
pub(crate) fn image(state: IconState, ink: [u8; 3]) -> tauri::image::Image<'static> {
    const SIZE: usize = 32;
    let covered = |x: f64, y: f64| -> bool {
        let distance = |cx: f64, cy: f64| (x - cx).hypot(y - cy);
        match state {
            IconState::Normal => (distance(8.0, 8.0) - 5.75).abs() <= 0.75,
            IconState::Awake => distance(8.0, 8.0) <= 6.5,
            IconState::Countdown => {
                distance(8.0, 8.0) <= 6.5 && (y <= 9.5 || (10.5..=12.0).contains(&y) || y >= 13.0)
            }
            IconState::Attention => {
                (distance(7.5, 8.5) <= 6.25 && distance(13.0, 3.0) >= 3.6)
                    || distance(13.0, 3.0) <= 2.1
            }
        }
    };
    let mut pixels = vec![0u8; SIZE * SIZE * 4];
    let scale = 16.0 / SIZE as f64;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut coverage = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let gx = (x as f64 + (sx as f64 + 0.5) / 4.0) * scale;
                    let gy = (y as f64 + (sy as f64 + 0.5) / 4.0) * scale;
                    coverage += u32::from(covered(gx, gy));
                }
            }
            if coverage > 0 {
                let i = (y * SIZE + x) * 4;
                pixels[i..i + 3].copy_from_slice(&ink);
                pixels[i + 3] = (coverage * 255 / 16) as u8;
            }
        }
    }
    tauri::image::Image::new_owned(pixels, SIZE as u32, SIZE as u32)
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
    use super::*;
    use crate::core::{
        countdown::{Countdown, Source},
        sessions::{Engine, Settings, Timer},
    };

    fn snapshot() -> Snapshot {
        Snapshot {
            actions: vec![PowerAction::Sleep],
            ..Snapshot::new("settings.json".into(), Settings::default())
        }
    }

    #[test]
    fn session_labels_round_up_to_whole_minutes() {
        assert_eq!(minutes_left(100, 100), "0m");
        assert_eq!(minutes_left(130, 100), "1m");
        assert_eq!(minutes_left(1900, 100), "30m");
        assert_eq!(minutes_left(3700, 100), "1h 0m");
        assert_eq!(minutes_left(7301, 100), "2h 1m");
    }

    #[test]
    fn menu_bar_title_prefers_the_warning_then_the_time_left_then_the_agent_count() {
        let mut snapshot = snapshot();
        assert_eq!(menu_bar_title(&snapshot), None);
        snapshot.engine.now = 100;
        snapshot.engine.awake_deadline = Some(100 + 2520);
        assert_eq!(menu_bar_title(&snapshot).as_deref(), Some("42m"));
        snapshot.engine.timer = Some(Timer {
            deadline: 100 + 6120,
            action: PowerAction::Sleep,
        });
        assert_eq!(menu_bar_title(&snapshot).as_deref(), Some("1:42"));
        snapshot.engine.countdown = Some(Countdown {
            deadline: 100 + 287,
            action: PowerAction::Sleep,
            source: Source::Timer,
        });
        assert_eq!(menu_bar_title(&snapshot).as_deref(), Some("4:47"));
        snapshot.settings.menu_bar_time = false;
        assert_eq!(menu_bar_title(&snapshot), None);
        // Older settings files gain the preferences switched on.
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert!(old.menu_bar_time && old.menu_bar_agent_count);
    }

    #[test]
    fn agent_count_shows_only_when_agents_alone_hold_the_mac() {
        let mut snapshot = snapshot();
        snapshot.settings.agents.trusted = vec!["claude-code".into(), "codex".into()];
        for (agent, id) in [("claude-code", "a"), ("codex", "b")] {
            crate::agents::hooks::call(
                &mut snapshot.engine,
                &snapshot.settings,
                "event",
                serde_json::json!({"agent": agent, "event": "UserPromptSubmit", "session_id": id}),
            )
            .unwrap();
        }
        assert_eq!(menu_bar_title(&snapshot).as_deref(), Some("2"));
        assert_eq!(icon_state_of(&snapshot), IconState::Awake);
        assert_eq!(status_detail(&snapshot), "For 2 agents");
        snapshot.settings.menu_bar_agent_count = false;
        assert_eq!(menu_bar_title(&snapshot), None);
        snapshot.settings.menu_bar_agent_count = true;
        snapshot.engine.keep_awake(None);
        assert_eq!(menu_bar_title(&snapshot), None);
        // A request waiting for approval shows the attention glyph.
        crate::agents::hooks::call(
            &mut snapshot.engine,
            &snapshot.settings,
            "event",
            serde_json::json!({"agent": "gemini-cli", "event": "BeforeAgent"}),
        )
        .unwrap();
        assert_eq!(icon_state_of(&snapshot), IconState::Attention);
    }

    #[test]
    fn clicks_open_the_panel_or_the_menu_as_set() {
        use MouseButton::{Left, Right};
        assert!(opens_panel(Left, false, IconClick::Panel));
        assert!(!opens_panel(Right, false, IconClick::Panel));
        assert!(!opens_panel(Left, true, IconClick::Panel));
        assert!(!opens_panel(Left, false, IconClick::Menu));
        assert!(opens_panel(Right, false, IconClick::Menu));
        assert!(opens_panel(Left, true, IconClick::Menu));
    }

    #[test]
    fn template_glyphs_distinguish_states_without_color() {
        let alpha =
            |state, x: usize, y: usize| image(state, [0, 0, 0]).rgba()[(y * 32 + x) * 4 + 3];
        // The ring is hollow; the suns are filled at the center.
        assert_eq!(alpha(IconState::Normal, 16, 16), 0);
        assert_eq!(alpha(IconState::Awake, 16, 16), 255);
        assert_eq!(alpha(IconState::Countdown, 16, 16), 255);
        // Only the setting sun has horizon gaps (y 9.5–10.5 in glyph units).
        assert_eq!(alpha(IconState::Awake, 16, 20), 255);
        assert_eq!(alpha(IconState::Countdown, 16, 20), 0);
        // The attention dot sits apart from its sun at the top right.
        assert_eq!(alpha(IconState::Attention, 26, 6), 255);
        assert_eq!(alpha(IconState::Attention, 20, 8), 0);
        // Windows inks the glyph for the taskbar.
        let white = image(IconState::Awake, [255, 255, 255]);
        assert_eq!(
            &white.rgba()[(16 * 32 + 16) * 4..][..4],
            &[255, 255, 255, 255]
        );
        let _ = Engine::default();
    }
}

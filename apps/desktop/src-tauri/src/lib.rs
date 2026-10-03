use tauri::{Manager, State};
mod cli;
mod core;
pub mod mcp;
mod menu_icons;
mod platform;
mod quick_settings;
mod state;
mod tray;

#[tauri::command]
fn panel_snapshot(state: State<'_, state::AppState>) -> Result<state::PanelSnapshot, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    state
        .sender
        .send(state::Request::PanelSnapshot(tx))
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(2))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn panel_action(
    action: String,
    session_id: Option<String>,
    state: State<'_, state::AppState>,
) -> Result<(), String> {
    let operation = match action.as_str() {
        "awake15" => state::Operation::KeepAwake { seconds: Some(900) },
        "awake30" => state::Operation::KeepAwake {
            seconds: Some(1800),
        },
        "awake60" => state::Operation::KeepAwake {
            seconds: Some(3600),
        },
        "awakeForever" => state::Operation::KeepAwake { seconds: None },
        "stopAwake" => state::Operation::StopAwake,
        "timer30" => state::Operation::ScheduleDefault,
        "playback" => state::Operation::TogglePlayback,
        "audio" => state::Operation::ToggleWhileAudio,
        "settings" => state::Operation::OpenDialog {
            view: state::DialogView::Settings,
        },
        "cancelSession" => state::Operation::CancelAgent {
            id: session_id.ok_or("Session id is required.")?,
        },
        _ => return Err("Unknown panel action.".into()),
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    state
        .sender
        .send(state::Request::Operation(operation, tx))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn panel_close(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("panel") {
        let _ = window.hide();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if cli::requested(&args) {
        std::process::exit(cli::main(&args));
    }
    if std::env::args().any(|arg| arg == "--mcp") {
        if let Err(error) = mcp::server::bridge() {
            eprintln!("Doze MCP: {error}");
            std::process::exit(1);
        }
        return;
    }
    let result = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            panel_snapshot,
            panel_action,
            panel_close
        ])
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            tray::show(app)
        }))
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let path = app.path().app_config_dir()?.join("settings.json");
            // A first launch shows Settings so people can find Doze in the menu bar.
            let first_run = !path.exists();
            let (settings, error) = match state::load(&path) {
                Ok(s) => (s, None),
                Err(e) => (core::sessions::Settings::default(), Some(e)),
            };
            if first_run {
                // Saving defaults makes the welcome a one-time event.
                let _ = state::persist(&path, &settings);
            }
            let at_login = std::env::args().any(|a| a == "--startup");
            let start_minimized = at_login || (settings.start_minimized && !first_run);
            app.manage(state::start(app.handle().clone(), path, settings, error)?);
            let state = app.state::<state::AppState>();
            // Agent connections are optional; their failure must never prevent the tray app.
            if let Err(error) = mcp::server::start(
                app.path().app_config_dir()?.join("mcp-endpoint.json"),
                state.sender.clone(),
            ) {
                eprintln!("Doze MCP bridge unavailable: {error}");
                let _ = state.sender.send(state::Request::AgentsUnavailable(format!(
                    "Agent connections unavailable: {error}"
                )));
            }
            tray::setup(app)?;
            if let Some(panel) = app.get_webview_window("panel") {
                let panel_for_close = panel.clone();
                panel.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::Focused(false)) {
                        let _ = panel_for_close.hide();
                    }
                });
            }
            if !start_minimized {
                tray::show(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .map(|app| {
            app.run(|_app, _event| {
                // Opening Doze again from Finder, Spotlight or Launchpad reactivates the running
                // menu bar app instead of launching a second instance. Show Settings, as a second
                // launch does on Windows.
                #[cfg(target_os = "macos")]
                if let tauri::RunEvent::Reopen { .. } = _event {
                    tray::show(_app);
                }
            })
        });
    if let Err(error) = result {
        eprintln!("Could not run Doze: {error}");
    }
}

use tauri::Manager;
mod cli;
mod core;
pub mod mcp;
mod menu_icons;
mod platform;
mod quick_settings;
mod state;
mod tray;

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

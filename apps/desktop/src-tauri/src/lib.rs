use tauri::Manager;
mod core;
pub mod mcp;
mod menu_icons;
mod platform;
mod quick_settings;
mod state;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            let (settings, error) = match state::load(&path) {
                Ok(s) => (s, None),
                Err(e) => (core::sessions::Settings::default(), Some(e)),
            };
            let start_minimized =
                settings.start_minimized || std::env::args().any(|a| a == "--startup");
            app.manage(state::start(app.handle().clone(), path, settings, error)?);
            let state = app.state::<state::AppState>();
            mcp::server::start(
                app.path().app_config_dir()?.join("mcp-endpoint.json"),
                state.sender.clone(),
            )?;
            tray::setup(app)?;
            if !start_minimized {
                tray::show(app.handle());
            }
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Could not run Doze: {error}");
    }
}

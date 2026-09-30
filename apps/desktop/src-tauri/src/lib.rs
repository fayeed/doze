use tauri::Manager;
mod core;
mod menu_icons;
mod platform;
mod quick_settings;
mod state;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            tray::show(app)
        }))
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("settings.json");
            let (settings, error) = match state::load(&path) {
                Ok(s) => (s, None),
                Err(e) => (core::sessions::Settings::default(), Some(e)),
            };
            let start_minimized =
                settings.start_minimized || std::env::args().any(|a| a == "--startup");
            app.manage(state::start(app.handle().clone(), path, settings, error)?);
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

mod model;
mod operations;
mod persistence;
mod runtime;

use crate::{
    core::sessions::{Engine, Settings},
    platform,
};
use runtime::worker;
use std::{
    path::PathBuf,
    sync::mpsc::{self, Sender},
};

pub use model::{DialogView, Operation, Preference, Request, Snapshot};
pub use persistence::{load, persist};

/// Commands enter a single-owner engine through this channel.
/// Native power requests and COM interfaces stay on the engine thread.
pub struct AppState {
    pub sender: Sender<Request>,
}

pub fn start(
    app: tauri::AppHandle,
    path: PathBuf,
    settings: Settings,
    initial_error: Option<String>,
) -> Result<AppState, String> {
    let (sender, receiver) = mpsc::channel();
    let snapshot = Snapshot {
        settings_path: path.clone(),
        selected_action: settings.default_action,
        view: DialogView::Settings,
        engine: Engine::default(),
        settings,
        actions: Vec::new(),
        audio_supported: platform::audio_supported(),
        startup_supported: platform::startup_supported(),
        error: initial_error,
    };
    let lifecycle_sender = sender.clone();
    std::thread::Builder::new()
        .name("doze-rules".into())
        .spawn(move || worker(app, path, snapshot, receiver, lifecycle_sender))
        .map_err(|error| format!("Could not start Doze engine: {error}"))?;
    Ok(AppState { sender })
}

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

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelSnapshot {
    pub awake: bool,
    pub awake_deadline: Option<u64>,
    pub now: u64,
    pub sessions: Vec<PanelSession>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelSession {
    pub id: String,
    pub provider: String,
    pub task: String,
    pub title: Option<String>,
    pub workspace: Option<String>,
    pub parent_session_id: Option<String>,
    pub activity: String,
    pub working_seconds: u64,
    pub started_at: u64,
    pub last_activity: u64,
    pub status: String,
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

//! Private pipes to the native platform UI. Rust owns settings and power actions.
use crate::state::{DialogView, Operation, Request, Snapshot};
use serde::Deserialize;
use serde_json::{json, Value};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Sender},
        Mutex, OnceLock,
    },
    time::Duration,
};

type Connection = (u64, Sender<Value>);
static UI: OnceLock<Mutex<Option<Connection>>> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UiRequest {
    command: String,
    settings: Option<crate::core::sessions::Settings>,
    #[cfg(target_os = "macos")]
    seconds: Option<u64>,
}

impl UiRequest {
    fn operation(self) -> Result<Operation, String> {
        Ok(match self.command.as_str() {
            "save" => Operation::SaveSettings {
                settings: self.settings.ok_or("Settings were not supplied.")?,
            },
            "preview" => Operation::PreviewCountdown,
            "refresh" => Operation::Refresh,
            "cancel" => Operation::Cancel,
            "snooze" => Operation::Snooze,
            #[cfg(target_os = "macos")]
            "quit" => Operation::Quit,
            #[cfg(target_os = "macos")]
            "awake" => Operation::KeepAwake {
                seconds: Some(self.seconds.ok_or("Duration missing.")?),
            },
            #[cfg(target_os = "macos")]
            "timer" => Operation::ScheduleSelected {
                seconds: self.seconds.ok_or("Duration missing.")?,
            },
            _ => return Err("Unknown native UI command.".into()),
        })
    }
}

fn reply_to_ui(line: &str, requests: &Sender<Request>) -> Result<Value, String> {
    let request: UiRequest = serde_json::from_str(line).map_err(|error| error.to_string())?;
    let saved = request.command == "save";
    let command = request.command.clone();
    let operation = request.operation()?;
    let (reply, response) = mpsc::channel();
    requests
        .send(Request::Operation(operation, reply))
        .map_err(|_| "Doze engine stopped.")?;
    let snapshot = response
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| "Doze engine did not respond.")??;
    Ok(json!({
        "type": if saved { "saved" } else { "state" },
        "command": command,
        "snapshot": snapshot_json(&snapshot),
    }))
}

fn snapshot_json(snapshot: &Snapshot) -> Value {
    let engine = &snapshot.engine;
    let timer = if let Some(countdown) = &engine.countdown {
        format!(
            "{} countdown · {} seconds remaining",
            countdown.action.label(),
            countdown.deadline.saturating_sub(engine.now)
        )
    } else if let Some(timer) = &engine.timer {
        format!(
            "{} in {} minutes",
            timer.action.label(),
            timer.deadline.saturating_sub(engine.now).div_ceil(60)
        )
    } else {
        "No power action scheduled".into()
    };
    json!({
        "settings": snapshot.settings,
        "settingsPath": snapshot.settings_path,
        "actions": snapshot.actions,
        "audioSupported": snapshot.audio_supported,
        "startupSupported": snapshot.startup_supported,
        "status": crate::tray::status_text(snapshot),
        "timerStatus": timer,
        "version": env!("CARGO_PKG_VERSION"),
    })
}

pub(super) fn show(snapshot: Snapshot, requests: Sender<Request>) -> Result<(), String> {
    let view = match snapshot.view {
        DialogView::Settings => "settings",
        DialogView::About => "about",
        DialogView::Help => "help",
        DialogView::AwakeDuration => "awakeDuration",
        DialogView::AwakeTime => "awakeTime",
        DialogView::TimerDuration => "timerDuration",
        DialogView::TimerTime => "timerTime",
    };
    send(
        json!({ "type": "open", "view": view, "snapshot": snapshot_json(&snapshot) }),
        requests,
    )
}

pub(super) fn send(open: Value, requests: Sender<Request>) -> Result<(), String> {
    let mut active = UI
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "Settings window state unavailable.")?;
    if let Some((_, queue)) = active.as_ref() {
        if queue.send(open.clone()).is_ok() {
            return Ok(());
        }
    }
    let executable = executable()?;
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // Suppress a console, not the native window.
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start native Settings: {error}"))?;
    let mut input = child
        .stdin
        .take()
        .ok_or("Settings input pipe unavailable.")?;
    let output = child
        .stdout
        .take()
        .ok_or("Settings output pipe unavailable.")?;
    let (queue, messages) = mpsc::channel::<Value>();
    queue.send(open).map_err(|error| error.to_string())?;
    std::thread::Builder::new()
        .name("doze-native-ui-write".into())
        .spawn(move || {
            for value in messages {
                if writeln!(input, "{value}")
                    .and_then(|_| input.flush())
                    .is_err()
                {
                    break;
                }
            }
        })
        .map_err(|error| error.to_string())?;
    let replies = queue.clone();
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed);
    std::thread::Builder::new()
        .name("doze-native-ui-read".into())
        .spawn(move || {
            let mut output = BufReader::new(output);
            loop {
                let mut line = String::new();
                // Bound messages from the UI; no network-facing endpoint exists.
                match output.by_ref().take(64 * 1024).read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if !line.ends_with('\n') => break,
                    Ok(_) => {}
                }
                let result = reply_to_ui(&line, &requests);
                let value = result.unwrap_or_else(|error| {
                    let command = serde_json::from_str::<UiRequest>(&line)
                        .ok()
                        .map(|request| request.command);
                    json!({ "type": "error", "command": command, "error": error })
                });
                if replies.send(value).is_err() {
                    break;
                }
            }
            let _ = child.wait();
            let _ = requests.send(Request::WarningFailed(
                "Native warning window closed unexpectedly.".into(),
            ));
            if let Ok(mut active) = UI.get_or_init(|| Mutex::new(None)).lock() {
                if active
                    .as_ref()
                    .is_some_and(|(current, _)| *current == generation)
                {
                    *active = None;
                }
            }
        })
        .map_err(|error| error.to_string())?;
    *active = Some((generation, queue));
    Ok(())
}

pub(super) fn executable() -> Result<std::path::PathBuf, String> {
    let current = std::env::current_exe().map_err(|error| error.to_string())?;
    let folder = current.parent().ok_or("Executable folder unavailable.")?;
    #[cfg(windows)]
    let (installed, development) = (
        folder.join("windows-ui/Doze.Settings.exe"),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../native/windows/publish/Doze.Settings.exe"),
    );
    #[cfg(target_os = "macos")]
    let (installed, development) = (
        folder.join("../Resources/macos-ui/Doze.NativeUI"),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../native/macos/publish/Doze.NativeUI"),
    );
    if installed.is_file() {
        return Ok(installed);
    }
    if cfg!(debug_assertions) && development.is_file() {
        return Ok(development);
    }
    // A locally built standalone release uses the same companion as the development checkout.
    if development.is_file() {
        return Ok(development);
    }
    Err("The native Settings companion is missing. Run pnpm --filter @doze/desktop native:build or reinstall Doze.".into())
}

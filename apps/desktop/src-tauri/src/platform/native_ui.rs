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
/// The companion's process id, for handing it the foreground.
static COMPANION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Clicking a tray icon lets its owner, the engine, bring a window to the foreground. Windows
/// refuses that to other background processes, so the engine passes the right on to the
/// companion: the flyout then becomes the active window and closes when focus moves away.
#[cfg(windows)]
fn allow_foreground() {
    use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
    let pid = COMPANION.load(Ordering::Relaxed);
    if pid != 0 {
        let _ = unsafe { AllowSetForegroundWindow(pid) };
    }
}
#[cfg(not(windows))]
fn allow_foreground() {}

/// What the open windows last received, without values that only count down.
static PUBLISHED: Mutex<String> = Mutex::new(String::new());

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UiRequest {
    command: String,
    settings: Option<crate::core::sessions::Settings>,
    id: Option<String>,
    decision: Option<String>,
    name: Option<String>,
    action: Option<crate::core::sessions::PowerAction>,
    agent_seconds: Option<u64>,
    seconds: Option<u64>,
    key: Option<String>,
    /// A setting's new value; JSON null clears optional settings.
    #[serde(default)]
    value: Value,
    agent: Option<String>,
    remove: Option<bool>,
    token: Option<String>,
    page: Option<String>,
    path: Option<std::path::PathBuf>,
}

impl UiRequest {
    fn operation(self) -> Result<Operation, String> {
        let id = |id: Option<String>| id.ok_or("Session missing.");
        Ok(match self.command.as_str() {
            "save" => Operation::SaveSettings {
                settings: self.settings.ok_or("Settings were not supplied.")?,
            },
            "set" => Operation::Set {
                key: self.key.ok_or("Setting missing.")?,
                value: self.value,
            },
            "agent-allow" => Operation::AuthorizeAgent {
                id: id(self.id)?,
                decision: "allow".into(),
            },
            "agent-deny" => Operation::AuthorizeAgent {
                id: id(self.id)?,
                decision: "deny".into(),
            },
            "agent-release" => Operation::CancelAgent { id: id(self.id)? },
            "connect-preview" => Operation::ConnectPreview {
                agent: self.agent.ok_or("Agent missing.")?,
                remove: self.remove.unwrap_or(false),
            },
            "connect-apply" => Operation::ConnectApply {
                agent: self.agent.ok_or("Agent missing.")?,
                remove: self.remove.unwrap_or(false),
                token: self.token.ok_or("Review the change first.")?,
            },
            "copy-config" => Operation::CopyConfig,
            "export-diagnostics" => Operation::ExportDiagnostics {
                path: self.path.ok_or("Choose where to save the report.")?,
            },
            "reset" => Operation::Reset,
            "open-settings" => Operation::OpenPage {
                page: self.page.unwrap_or_else(|| "overview".into()),
            },
            "open-timer" => Operation::OpenDialog {
                view: match self.name.as_deref() {
                    Some("awakeTime") => DialogView::AwakeTime,
                    Some("timerDuration") => DialogView::TimerDuration,
                    Some("timerTime") => DialogView::TimerTime,
                    _ => DialogView::AwakeDuration,
                },
            },
            "preview" => Operation::PreviewCountdown,
            "refresh" => Operation::Refresh,
            "agent-enable" => Operation::AgentEnabled,
            "agent-keepalive" => Operation::AgentKeepAlive,
            "agent-connect" => Operation::ConnectAgent {
                name: self.name.ok_or("Client name missing.")?,
            },
            "agent-skill-install" => Operation::InstallAgentSkill {
                name: self.name.ok_or("Client name missing.")?,
                update: false,
            },
            "agent-skill-update" => Operation::InstallAgentSkill {
                name: self.name.ok_or("Client name missing.")?,
                update: true,
            },
            "agent-skill-folder" => Operation::OpenAgentSkillFolder,
            "agent-authorize" => Operation::AuthorizeAgent {
                id: self.id.ok_or("Session missing.")?,
                decision: self.decision.ok_or("Decision missing.")?,
            },
            "agent-cancel" => Operation::CancelAgent {
                id: self.id.ok_or("Session missing.")?,
            },
            "agent-wait" => Operation::WaitAgent {
                id: self.id.ok_or("Session missing.")?,
            },
            "agent-finish" => Operation::FinishAgent {
                id: self.id.ok_or("Session missing.")?,
            },
            "agent-revoke" => Operation::RevokeAgent {
                id: self.id.ok_or("Client missing.")?,
            },
            "agent-permission" => Operation::AgentPermission {
                id: self.id.ok_or("Client missing.")?,
                action: self.action,
            },
            "agent-lease" => Operation::AgentLease {
                seconds: self.agent_seconds.ok_or("Lease duration missing.")?,
            },
            "agent-default" => Operation::AgentDefault {
                action: self.action,
            },
            "cancel" => Operation::Cancel,
            "stay-awake" => Operation::StayAwake,
            "snooze" => Operation::Snooze,
            "quit" => Operation::Quit,
            "awake" => Operation::KeepAwake {
                seconds: Some(self.seconds.ok_or("Duration missing.")?),
            },
            "timer" => match self.action {
                Some(action) => Operation::Schedule {
                    seconds: self.seconds.ok_or("Duration missing.")?,
                    action,
                },
                None => Operation::ScheduleSelected {
                    seconds: self.seconds.ok_or("Duration missing.")?,
                },
            },
            "awake-forever" => Operation::KeepAwake { seconds: None },
            "awake-default" => Operation::KeepAwakeDefault,
            "timer-default" => Operation::ScheduleDefault,
            "stop-awake" => Operation::StopAwake,
            "extend" => Operation::ExtendAwake { seconds: 900 },
            "stop-timer" => Operation::StopTimer,
            "audio-toggle" => Operation::ToggleWhileAudio,
            "playback-toggle" => Operation::TogglePlayback,
            "select-action" => Operation::SelectAction {
                action: self.action.ok_or("Action missing.")?,
            },
            _ => return Err("Unknown native UI command.".into()),
        })
    }
}

fn reply_to_ui(line: &str, requests: &Sender<Request>) -> Result<Value, String> {
    let request: UiRequest = serde_json::from_str(line).map_err(|error| error.to_string())?;
    let saved = matches!(request.command.as_str(), "save" | "set" | "reset");
    let command = request.command.clone();
    // Replies that change files or connections carry the full snapshot.
    let full = matches!(
        command.as_str(),
        "refresh"
            | "connect-apply"
            | "copy-config"
            | "reset"
            | "agent-skill-install"
            | "agent-skill-update"
            | "agent-connect"
    );
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
        "result": snapshot.result,
        "snapshot": snapshot_json(&snapshot, full),
    }))
}

/// Sends the engine's state to the open windows when it changed. Values that only count
/// down are left out of the comparison: windows add their own elapsed time to `now`.
pub fn publish(snapshot: &Snapshot) {
    let connected = UI
        .get()
        .and_then(|ui| ui.lock().ok())
        .is_some_and(|ui| ui.is_some());
    if !connected {
        return;
    }
    let value = snapshot_json(snapshot, false);
    let mut stable = value.clone();
    for key in ["now", "agentNow", "timerStatus"] {
        stable.as_object_mut().map(|o| o.remove(key));
    }
    if let Some(session) = stable["session"].as_object_mut() {
        session.remove("awakeRemaining");
        for key in ["timer", "countdown"] {
            if let Some(part) = session.get_mut(key).and_then(Value::as_object_mut) {
                part.remove("remaining");
            }
        }
    }
    let signature = stable.to_string();
    let Ok(mut published) = PUBLISHED.lock() else {
        return;
    };
    if *published == signature {
        return;
    }
    if let Ok(ui) = UI.get_or_init(|| Mutex::new(None)).lock() {
        if let Some((_, queue)) = ui.as_ref() {
            if queue
                .send(json!({ "type": "state", "snapshot": value }))
                .is_ok()
            {
                *published = signature;
            }
        }
    }
}

/// Live session state for the native control center. Times are seconds remaining, with the
/// engine's own deadlines beside them.
fn session_json(snapshot: &Snapshot) -> Value {
    use crate::core::sessions::Phase;
    let engine = &snapshot.engine;
    let remaining = |deadline: u64| deadline.saturating_sub(engine.now);
    json!({
        "awake": engine.awake,
        "awakeDeadline": engine.awake_deadline,
        "awakeRemaining": engine.awake_deadline.map(remaining),
        "whileAudio": engine.while_audio,
        // Whether sound reaches the output, sampled only while an audio rule is on.
        "audioActive": engine.audio_active,
        "holdingAwake": engine.should_hold_awake(),
        "playbackEnabled": engine.playback_enabled,
        "playbackPhase": match engine.playback_phase {
            Phase::Idle | Phase::Cancelled | Phase::Completed => "waiting",
            Phase::Active => "playing",
            Phase::GracePeriod => "grace",
            Phase::Countdown => "countdown",
        },
        "selectedAction": snapshot.selected_action,
        "timer": engine.timer.as_ref().map(|timer| json!({
            "action": timer.action,
            "deadline": timer.deadline,
            "remaining": remaining(timer.deadline),
        })),
        "countdown": engine.countdown.as_ref().map(|countdown| json!({
            "action": countdown.action,
            "deadline": countdown.deadline,
            "remaining": remaining(countdown.deadline),
            "source": countdown.source,
            "length": match countdown.source {
                crate::core::countdown::Source::Agents => snapshot.settings.agent_warning_seconds(),
                _ => snapshot.settings.countdown_seconds,
            },
        })),
        "batteryLow": engine.battery_low,
        "error": snapshot.error,
        // The latest engine event, such as a cancelled action or sessions cleared by sleep.
        "message": engine.message,
    })
}

/// What the panel and menu bar show about agents.
fn agents_json(snapshot: &Snapshot) -> Value {
    let sessions = crate::agents::visible(&snapshot.engine.agents);
    let count = |state| sessions.iter().filter(|s| s.state == state).count();
    use crate::agents::AgentState;
    json!({
        "sessions": sessions,
        "working": count(AgentState::Working),
        "idle": count(AgentState::Idle),
        "pending": count(AgentState::NeedsApproval),
        "done": count(AgentState::Done),
    })
}

/// The snapshot every window renders. `full` adds parts that read files, such as the agent
/// connections; streamed updates leave them out and windows keep the last ones.
fn snapshot_json(snapshot: &Snapshot, full: bool) -> Value {
    let mut value = base_json(snapshot);
    if full {
        let home = crate::agents::connect::home().unwrap_or_default();
        value["agentLinks"] = json!(crate::agents::connect::connections(&home));
        value["agentSkills"] = crate::mcp::skill::states();
        value["agentConnections"] =
            crate::mcp::server::connection_configs(&snapshot.settings, &snapshot.settings_path);
    }
    value
}

fn base_json(snapshot: &Snapshot) -> Value {
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
        "session": session_json(snapshot),
        "now": snapshot.engine.now,
        "agents": agents_json(snapshot),
        "agentSessions": snapshot.engine.agents.items,
        "agentNow": snapshot.engine.now,
        "agentSkillMessage": snapshot.engine.message,
        "settingsPath": snapshot.settings_path,
        "actions": snapshot.actions,
        "audioSupported": snapshot.audio_supported,
        "startupSupported": snapshot.startup_supported,
        // Laptops only: computers without a lid have nothing to keep open.
        "lidClosedSupported": snapshot.lid_supported,
        "status": crate::tray::status_text(snapshot),
        "statusDetail": crate::tray::status_detail(snapshot),
        "statusShort": crate::tray::status_short(snapshot),
        "iconState": crate::tray::icon_state(snapshot),
        "timerStatus": timer,
        "battery": snapshot.engine.battery.map(|(percent, on_battery)| json!({
            "percent": percent,
            "onBattery": on_battery,
        })),
        "assertions": snapshot.assertions,
        "mcpAddress": crate::mcp::server::address().map(|a| a.to_string()),
        "version": env!("CARGO_PKG_VERSION"),
        "links": crate::links::json(),
        // Shown in Settings so `doze run` can be copied with the right path.
        "executable": std::env::current_exe().ok(),
        "cliPath": crate::agents::connect::executable(),
        "iconPath": icon_path(),
        "clypyIconPath": resource_path("macos-ui/clypy.png", "icons/clypy.png"),
        "glyphPaths": glyph_paths(),
    })
}

/// The brand template glyphs for the Menu guide, by state.
fn glyph_paths() -> Value {
    let mut glyphs = serde_json::Map::new();
    for state in ["normal", "awake", "attention", "countdown"] {
        let file = format!("doze-glyph-{state}.svg");
        let path = resource_path(&format!("glyphs/{file}"), &format!("icons/glyphs/{file}"));
        glyphs.insert(state.into(), json!(path));
    }
    Value::Object(glyphs)
}

/// A bundled resource, or the checkout's copy during development.
fn resource_path(bundled: &str, development: &str) -> Option<std::path::PathBuf> {
    let installed = std::env::current_exe()
        .ok()?
        .parent()?
        .join("../Resources")
        .join(bundled);
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(development);
    [installed, checkout]
        .into_iter()
        .find(|path| path.is_file())
}

/// Doze's app icon for the native Settings sidebar and About page: the bundle's icon when
/// installed, otherwise the checkout's.
fn icon_path() -> Option<std::path::PathBuf> {
    let bundled = std::env::current_exe()
        .ok()?
        .parent()?
        .join("../Resources/icon.icns");
    let development = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/128x128@2x.png");
    [bundled, development]
        .into_iter()
        .find(|path| path.is_file())
}

pub(super) fn show(snapshot: Snapshot, requests: Sender<Request>) -> Result<(), String> {
    if snapshot.view == DialogView::Panel {
        // Before the message, so the right is in place when the companion activates the
        // flyout; again after it, for a companion this call has just started.
        allow_foreground();
        send(
            json!({
                "type": "panel",
                "anchor": snapshot.panel_anchor,
                "snapshot": snapshot_json(&snapshot, false),
            }),
            requests,
        )?;
        allow_foreground();
        return Ok(());
    }
    let view = match snapshot.view {
        DialogView::Panel => "panel",
        DialogView::Settings => "settings",
        DialogView::Agents => "agents",
        DialogView::About => "about",
        DialogView::Help => "help",
        DialogView::AwakeDuration => "awakeDuration",
        DialogView::AwakeTime => "awakeTime",
        DialogView::TimerDuration => "timerDuration",
        DialogView::TimerTime => "timerTime",
    };
    send(
        json!({
            "type": "open",
            "view": view,
            "page": snapshot.page,
            "snapshot": snapshot_json(&snapshot, true),
        }),
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
    COMPANION.store(child.id(), Ordering::Relaxed);
    // A new window process has seen nothing yet.
    if let Ok(mut published) = PUBLISHED.lock() {
        published.clear();
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::sessions::PowerAction;

    fn operation(line: &str) -> Result<Operation, String> {
        serde_json::from_str::<UiRequest>(line)
            .map_err(|error| error.to_string())?
            .operation()
    }

    #[test]
    fn control_center_commands_map_to_engine_operations() {
        assert!(matches!(
            operation(r#"{"command":"awake-forever"}"#),
            Ok(Operation::KeepAwake { seconds: None })
        ));
        assert!(matches!(
            operation(r#"{"command":"extend"}"#),
            Ok(Operation::ExtendAwake { seconds: 900 })
        ));
        assert!(matches!(
            operation(r#"{"command":"timer","seconds":600,"action":"displayOff"}"#),
            Ok(Operation::Schedule {
                seconds: 600,
                action: PowerAction::DisplayOff
            })
        ));
        assert!(matches!(
            operation(r#"{"command":"timer","seconds":600}"#),
            Ok(Operation::ScheduleSelected { seconds: 600 })
        ));
        assert!(operation(r#"{"command":"select-action"}"#).is_err());
        assert!(matches!(
            operation(r#"{"command":"set","key":"stayAwakeMinutes","value":null}"#),
            Ok(Operation::Set {
                value: Value::Null,
                ..
            })
        ));
        assert!(operation(r#"{"command":"set","value":1}"#).is_err());
        assert!(operation(r#"{"command":"format-disk"}"#).is_err());
    }
}

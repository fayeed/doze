//! Settings › Advanced › Export a diagnostics report. Written only to a file the user picks;
//! agent credentials are redacted and nothing leaves the computer.
use super::Snapshot;
use serde_json::{json, Value};

pub(super) fn report(snapshot: &Snapshot) -> Value {
    let mut settings = json!(snapshot.settings);
    if let Some(clients) = settings["agents"]["clients"].as_array_mut() {
        for client in clients {
            client["secret"] = json!("(redacted)");
        }
    }
    let log = std::fs::read(snapshot.settings_path.with_extension("log"))
        .map(|bytes| {
            let start = bytes.len().saturating_sub(64 * 1024);
            String::from_utf8_lossy(&bytes[start..]).into_owned()
        })
        .unwrap_or_default();
    let engine = &snapshot.engine;
    json!({
        "app": "Doze",
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "status": crate::tray::status_text(snapshot),
        "error": snapshot.error,
        "message": engine.message,
        "holdingAwake": engine.should_hold_awake(),
        "assertions": snapshot.assertions,
        "battery": engine.battery,
        "batteryLow": engine.battery_low,
        "awake": engine.awake,
        "awakeRemaining": engine.awake_deadline.map(|d| d.saturating_sub(engine.now)),
        "timer": engine.timer,
        "countdown": engine.countdown,
        "playbackEnabled": engine.playback_enabled,
        "playbackPhase": engine.playback_phase,
        "sessions": crate::agents::visible(&engine.agents),
        "supportedActions": snapshot.actions,
        "settings": settings,
        "log": log,
    })
}

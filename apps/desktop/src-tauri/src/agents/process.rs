//! Tools without lifecycle hooks, such as Cursor, are detected by their running process.
//! They show as Open and hold the computer awake only when the user turns that on per tool.
use super::{AgentKind, SessionSource};
use crate::{
    core::sessions::{Engine, Settings},
    mcp::sessions::{Activity, Session, Status},
};

/// The tools Doze can detect, with their process names on macOS and Windows.
pub const TOOLS: [(&str, &[&str]); 1] = [("cursor", &["Cursor", "Cursor.exe"])];

/// Whether any detection is on, so the engine only lists processes when it matters.
pub fn wanted(settings: &Settings) -> bool {
    settings.agents.enabled && settings.agents.process_tools.iter().any(|t| t.detect)
}

/// Tool ids whose process is among `names`.
pub fn detect(names: &[String]) -> Vec<&'static str> {
    TOOLS
        .iter()
        .filter(|(_, processes)| {
            names
                .iter()
                .any(|name| processes.iter().any(|p| name.eq_ignore_ascii_case(p)))
        })
        .map(|(id, _)| *id)
        .collect()
}

/// Brings process sessions in line with what is running and the per-tool settings.
pub fn observe(engine: &mut Engine, settings: &Settings, running: &[&str]) {
    let now = engine.now;
    for (id, _) in TOOLS {
        let tool = settings
            .agents
            .process_tools
            .iter()
            .find(|t| t.id == id && t.detect)
            .filter(|_| settings.agents.enabled);
        let open = engine.agents.items.iter().position(|s| {
            s.source == SessionSource::Process && s.client_id == id && !s.status.terminal()
        });
        match (tool, running.contains(&id), open) {
            (Some(tool), true, Some(index)) => {
                let session = &mut engine.agents.items[index];
                session.last_heartbeat = now;
                session.set_activity(
                    if tool.keep_awake {
                        Activity::Working
                    } else {
                        Activity::Idle
                    },
                    now,
                );
            }
            (Some(tool), true, None) => {
                let agent = AgentKind::parse(id).unwrap_or(AgentKind::Custom(id.into()));
                engine.agents.items.push(Session {
                    session_id: uuid::Uuid::new_v4().to_string(),
                    client_id: id.into(),
                    client_name: agent.label(),
                    agent,
                    source: SessionSource::Process,
                    project: None,
                    reason: "Detected by process".into(),
                    title: None,
                    workspace: None,
                    activity: if tool.keep_awake {
                        Activity::Working
                    } else {
                        Activity::Idle
                    },
                    parent_session_id: None,
                    provider_session_id: None,
                    activity_changed_at: now,
                    working_seconds: 0,
                    created_at: now,
                    last_heartbeat: now,
                    // A running process is its own check-in.
                    lease_expires_at: u64::MAX,
                    completion_action: None,
                    authorized_action: None,
                    status: Status::Active,
                    timeout_at: None,
                    lost_at: None,
                    explicit_at: now,
                    explicit_action: false,
                });
            }
            // Closing the tool, or turning detection off, ends the session without an action.
            (_, _, Some(index)) => engine.agents.items[index].status = Status::Cancelled,
            _ => {}
        }
    }
}

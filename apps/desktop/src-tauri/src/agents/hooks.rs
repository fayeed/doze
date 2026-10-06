//! `doze hook <agent> <event>` forwards an agent's native lifecycle event here, over the same
//! private bridge as `doze run`. Each tool's event names map onto one small vocabulary.
use super::{AgentKind, SessionSource};
use crate::{
    core::sessions::{Engine, Event, Settings},
    mcp::{
        sessions::{Activity, Session, Status, LOST_GRACE_SECONDS},
        tools::join_authorized_batch,
    },
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    /// The agent process or conversation started. Nothing is held yet.
    Start,
    /// The user submitted a prompt: the turn is working.
    Prompt,
    /// Tool use or model output inside a turn: still working, and a check-in.
    Activity,
    /// Waiting on the user mid-turn, such as a permission prompt. Still keeps awake.
    Waiting,
    /// The turn completed.
    Idle,
    /// The session ended.
    End,
}

/// Native event names per tool. Unknown events are ignored so new hook types never break
/// an agent. Generic names work for every agent, including custom ones.
pub fn classify(agent: &AgentKind, event: &str) -> Option<Lifecycle> {
    use Lifecycle::*;
    let generic = match event.to_ascii_lowercase().as_str() {
        "start" => Some(Start),
        "working" | "prompt" => Some(Prompt),
        "activity" | "heartbeat" => Some(Activity),
        "waiting" => Some(Waiting),
        "idle" | "stop" | "done" => Some(Idle),
        "end" => Some(End),
        _ => None,
    };
    let native = match agent {
        // Claude Code: https://docs.claude.com/en/docs/claude-code/hooks
        AgentKind::ClaudeCode => match event {
            "SessionStart" => Some(Start),
            "UserPromptSubmit" => Some(Prompt),
            "PreToolUse" | "PostToolUse" | "PostToolBatch" | "SubagentStart" | "SubagentStop"
            | "PreCompact" | "PostCompact" => Some(Activity),
            // Waiting on the user mid-task: keep the machine awake.
            "Notification" | "PermissionRequest" | "Elicitation" => Some(Waiting),
            "Stop" | "StopFailure" => Some(Idle),
            "SessionEnd" => Some(End),
            _ => None,
        },
        // Codex copies Claude Code's hook vocabulary; `notify` sends agent-turn-complete.
        AgentKind::Codex => match event {
            "SessionStart" => Some(Start),
            "UserPromptSubmit" => Some(Prompt),
            "PreToolUse" | "PostToolUse" | "SubagentStart" | "SubagentStop" | "PreCompact"
            | "PostCompact" => Some(Activity),
            "PermissionRequest" => Some(Waiting),
            "Stop" | "Interrupt" | "agent-turn-complete" => Some(Idle),
            "SessionEnd" => Some(End),
            _ => None,
        },
        // Gemini CLI: https://geminicli.com/docs/hooks/
        AgentKind::GeminiCli => match event {
            "SessionStart" => Some(Start),
            "BeforeAgent" => Some(Prompt),
            "BeforeModel"
            | "AfterModel"
            | "BeforeToolSelection"
            | "BeforeTool"
            | "AfterTool"
            | "PreCompress" => Some(Activity),
            "Notification" => Some(Waiting),
            "AfterAgent" => Some(Idle),
            "SessionEnd" => Some(End),
            _ => None,
        },
        // OpenCode, from Doze's plugin: https://opencode.ai/docs/plugins/
        AgentKind::OpenCode => match event {
            "session.created" => Some(Start),
            "chat.message" | "session.status:busy" | "session.status:retry" => Some(Prompt),
            "tool.execute.before" | "tool.execute.after" | "message.updated" => Some(Activity),
            "permission.asked" | "permission.updated" => Some(Waiting),
            "session.idle" | "session.status:idle" | "session.error" => Some(Idle),
            "session.deleted" => Some(End),
            _ => None,
        },
        _ => None,
    };
    native.or(generic)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HookInput {
    agent: String,
    event: String,
    session_id: Option<String>,
    cwd: Option<String>,
    prompt: Option<String>,
}

fn clean(value: Option<String>, limit: usize) -> Option<String> {
    value
        .map(|v| {
            v.chars()
                .filter(|c| !c.is_control())
                .take(limit)
                .collect::<String>()
        })
        .filter(|v| !v.trim().is_empty())
}

/// Applies one lifecycle event. Returns the session as the apps render it.
pub fn call(
    engine: &mut Engine,
    settings: &Settings,
    name: &str,
    arguments: Value,
) -> Result<Value, String> {
    if name != "event" {
        return Err("Unknown hook command.".into());
    }
    let input: HookInput =
        serde_json::from_value(arguments).map_err(|e| format!("Invalid hook event: {e}"))?;
    if !settings.agents.enabled {
        return Err("Agents are turned off in Doze.".into());
    }
    let agent = AgentKind::parse(&input.agent)?;
    if matches!(agent, AgentKind::McpClient(_) | AgentKind::Cursor) {
        return Err("This agent cannot send hook events.".into());
    }
    let Some(lifecycle) = classify(&agent, &input.event) else {
        return Ok(json!({ "ignored": input.event }));
    };
    let cwd = clean(input.cwd, 1024);
    // Without a session id, one session per project folder.
    let key = clean(input.session_id, 256)
        .or_else(|| cwd.clone())
        .unwrap_or_else(|| "default".into());
    let prompt = input.prompt.as_deref().and_then(super::summary);
    apply(engine, settings, agent, &key, cwd, prompt, lifecycle)
}

pub fn apply(
    engine: &mut Engine,
    settings: &Settings,
    agent: AgentKind,
    key: &str,
    cwd: Option<String>,
    prompt: Option<String>,
    lifecycle: Lifecycle,
) -> Result<Value, String> {
    let now = engine.now;
    let client_id = agent.id();
    // A denied session stays denied: its later events neither hold nor ask again.
    if engine.agents.items.iter().any(|s| {
        s.client_id == client_id
            && s.provider_session_id.as_deref() == Some(key)
            && s.status == Status::Denied
    }) {
        return Ok(json!({ "ignored": "denied" }));
    }
    let found = engine.agents.items.iter().position(|s| {
        s.client_id == client_id
            && s.provider_session_id.as_deref() == Some(key)
            && !s.status.terminal()
    });
    let index = match (found, lifecycle) {
        (None, Lifecycle::End) => return Ok(json!({ "ignored": "no open session" })),
        (Some(index), Lifecycle::End) => {
            let session = &mut engine.agents.items[index];
            session.set_activity(Activity::Idle, now);
            session.last_heartbeat = now;
            // Ending a request that was never allowed is not a completion.
            session.status = if session.status == Status::AwaitingAuthorization {
                Status::Cancelled
            } else {
                Status::Finished
            };
            return Ok(json!(super::view(session)));
        }
        (Some(index), _) => index,
        (None, _) => {
            if engine
                .agents
                .items
                .iter()
                .filter(|s| !s.status.terminal())
                .count()
                >= 64
            {
                return Err("Too many agent sessions.".into());
            }
            let trusted = settings.agents.is_trusted(&client_id);
            let session = Session {
                session_id: uuid::Uuid::new_v4().to_string(),
                client_id: client_id.clone(),
                client_name: agent.label(),
                source: if agent == AgentKind::OpenCode {
                    SessionSource::Plugin
                } else {
                    SessionSource::Hooks
                },
                project: cwd.as_deref().and_then(super::project),
                agent: agent.clone(),
                reason: String::new(),
                title: None,
                workspace: cwd.clone(),
                activity: Activity::Idle,
                parent_session_id: None,
                provider_session_id: Some(key.into()),
                activity_changed_at: now,
                working_seconds: 0,
                created_at: now,
                last_heartbeat: now,
                lease_expires_at: now.saturating_add(LOST_GRACE_SECONDS),
                completion_action: None,
                authorized_action: None,
                status: if trusted {
                    Status::Active
                } else {
                    Status::AwaitingAuthorization
                },
                timeout_at: None,
                lost_at: None,
                explicit_at: now,
                explicit_action: false,
            };
            if !trusted {
                engine.events.push(Event::ApprovalNeeded {
                    agent: agent.label(),
                    project: session.project.clone(),
                });
                crate::mcp::tools::prune_history(engine);
            }
            engine.agents.items.push(session);
            engine.agents.items.len() - 1
        }
    };
    let session = &mut engine.agents.items[index];
    let was_holding = session.holds();
    session.last_heartbeat = now;
    session.explicit_at = now;
    session.lease_expires_at = now.saturating_add(LOST_GRACE_SECONDS);
    if let Some(cwd) = cwd {
        session.project = super::project(&cwd);
        session.workspace = Some(cwd);
    }
    if let Some(prompt) = prompt {
        session.reason = prompt;
    }
    match lifecycle {
        Lifecycle::Prompt | Lifecycle::Activity => session.set_activity(Activity::Working, now),
        Lifecycle::Waiting => session.set_activity(Activity::Waiting, now),
        Lifecycle::Idle => session.set_activity(Activity::Idle, now),
        Lifecycle::Start | Lifecycle::End => {}
    }
    // Any event is a check-in: a session in its missed check-in grace is back.
    if session.status == Status::ConnectionLost {
        session.status = Status::Active;
        session.lost_at = None;
    }
    let id = session.session_id.clone();
    if !was_holding && session.holds() {
        join_authorized_batch(engine, &id);
    }
    let session = engine
        .agents
        .items
        .iter()
        .find(|s| s.session_id == id)
        .ok_or("Session not found.")?;
    Ok(json!(super::view(session)))
}

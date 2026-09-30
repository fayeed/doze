use crate::{
    core::{
        countdown::Source,
        sessions::{Engine, PowerAction, Settings},
    },
    mcp::{
        auth::TrustedClient,
        sessions::{Session, Status},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub key: String,
    pub name: String,
    pub arguments: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    reason: String,
    completion_action: Option<String>,
    optional_timeout: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Id {
    session_id: String,
}
fn parse_action(
    value: Option<&str>,
    default: Option<PowerAction>,
) -> Result<Option<PowerAction>, String> {
    Ok(match value {
        None => default,
        Some("return_to_normal") => None,
        Some("sleep") => Some(PowerAction::Sleep),
        Some("hibernate") => Some(PowerAction::Hibernate),
        Some("lock") => Some(PowerAction::Lock),
        Some("display_off") => Some(PowerAction::DisplayOff),
        Some("shutdown") => Some(PowerAction::Shutdown),
        _ => return Err("Unknown completion_action.".into()),
    })
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("Invalid arguments: {e}"))
}
pub fn call(
    engine: &mut Engine,
    settings: &Settings,
    supported: &[PowerAction],
    call: Call,
) -> Result<Value, String> {
    let client = settings.agents.authenticate(&call.key)?;
    if call.name == "doze.start_session" {
        let start: Start = decode(call.arguments)?;
        if start.reason.trim().is_empty()
            || start.reason.len() > 512
            || start.reason.chars().any(char::is_control)
        {
            return Err("Reason must be 1–512 bytes without control characters.".into());
        }
        if start
            .optional_timeout
            .is_some_and(|t| !(30..=604800).contains(&t))
        {
            return Err("optional_timeout must be 30–604800 seconds.".into());
        }
        let action = parse_action(
            start.completion_action.as_deref(),
            settings.agents.default_completion,
        )?;
        if action.is_some_and(|a| !supported.contains(&a)) {
            return Err("Unsupported completion action.".into());
        }
        if engine.agents.batch.len() >= 256 && engine.agents.unsettled() {
            return Err("Settle the current batch before starting more agent sessions.".into());
        }
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
        let approved = client.keep_awake && action.is_none_or(|a| client.actions.contains(&a));
        let session = Session {
            session_id: uuid::Uuid::new_v4().to_string(),
            client_id: client.id.clone(),
            client_name: client.name.clone(),
            reason: start.reason,
            created_at: engine.now,
            last_heartbeat: engine.now,
            lease_expires_at: engine
                .now
                .saturating_add(settings.agents.lease_seconds)
                .min(
                    start
                        .optional_timeout
                        .map_or(u64::MAX, |t| engine.now.saturating_add(t)),
                ),
            completion_action: action,
            authorized_action: if approved { action } else { None },
            status: if approved {
                Status::Active
            } else {
                Status::AwaitingAuthorization
            },
            timeout_at: start.optional_timeout.map(|t| engine.now.saturating_add(t)),
        };
        if approved {
            join_authorized_batch(engine, &session.session_id);
        } else {
            prune_history(engine);
        }
        let result = json!(session);
        engine.agents.items.push(session);
        return Ok(result);
    }
    if call.name == "doze.list_sessions" {
        if !call.arguments.as_object().is_some_and(|o| o.is_empty()) {
            return Err("No arguments expected.".into());
        }
        return Ok(json!(engine
            .agents
            .items
            .iter()
            .filter(|s| s.client_id == client.id)
            .collect::<Vec<_>>()));
    }
    if ![
        "doze.heartbeat",
        "doze.finish_session",
        "doze.fail_session",
        "doze.cancel_session",
        "doze.get_session",
    ]
    .contains(&call.name.as_str())
    {
        return Err("Unknown tool.".into());
    }
    let id: Id = decode(call.arguments)?;
    // A guessed session id never grants access across configured clients.
    let session = engine
        .agents
        .items
        .iter_mut()
        .find(|s| s.session_id == id.session_id && s.client_id == client.id)
        .ok_or("Session not found for this client.")?;
    match call.name.as_str() {
        "doze.heartbeat" => {
            if !session.status.holds_awake() {
                return Err("Session is not active.".into());
            }
            if session.timeout_at.is_some_and(|t| engine.now >= t) {
                return Err("Session timeout reached. Resolve in Doze.".into());
            }
            session.status = Status::Active;
            session.last_heartbeat = engine.now;
            session.lease_expires_at = engine
                .now
                .saturating_add(settings.agents.lease_seconds)
                .min(session.timeout_at.unwrap_or(u64::MAX));
        }
        "doze.finish_session" => {
            if session.status != Status::Finished {
                if !session.status.holds_awake() {
                    return Err("Session is not authorized or already ended.".into());
                }
                session.status = Status::Finished;
            }
        }
        "doze.fail_session" => {
            if session.status != Status::Failed {
                if !session.status.holds_awake() {
                    return Err("Session is not authorized or already ended.".into());
                }
                session.status = Status::Failed;
            }
        }
        "doze.cancel_session" => {
            if matches!(session.status, Status::Finished | Status::Failed) {
                return Err("Session already ended; cancel the countdown in Doze.".into());
            }
            if session.status != Status::Denied {
                session.status = Status::Cancelled;
            }
        }
        "doze.get_session" => {}
        _ => return Err("Unknown tool.".into()),
    }
    Ok(json!(session))
}

pub fn authorize(
    engine: &mut Engine,
    settings: &mut Settings,
    id: &str,
    decision: &str,
) -> Result<(), String> {
    if !settings.agents.enabled {
        return Err("MCP is disabled.".into());
    }
    if !["once", "deny"].contains(&decision) {
        return Err("Persistent permissions can only be changed in Agents settings.".into());
    }
    let session = engine
        .agents
        .items
        .iter()
        .find(|s| s.session_id == id)
        .ok_or("Session not found.")?;
    if session.status != Status::AwaitingAuthorization {
        return Err("Authorization is no longer pending.".into());
    }
    if !settings
        .agents
        .clients
        .iter()
        .any(|c| c.id == session.client_id)
    {
        return Err("Client revoked.".into());
    }
    if decision == "once" {
        join_authorized_batch(engine, id);
    }
    let session = engine
        .agents
        .items
        .iter_mut()
        .find(|s| s.session_id == id)
        .ok_or("Session not found.")?;
    if decision == "deny" {
        session.status = Status::Denied;
        return Ok(());
    }
    session.status = Status::Active;
    session.authorized_action = session.completion_action;
    session.last_heartbeat = engine.now;
    session.lease_expires_at = engine
        .now
        .saturating_add(settings.agents.lease_seconds)
        .min(session.timeout_at.unwrap_or(u64::MAX));
    Ok(())
}
pub fn connect(settings: &mut Settings, name: &str) -> Result<(), String> {
    if !["Codex", "Claude Code", "Generic MCP client"].contains(&name) {
        return Err("Unknown client profile.".into());
    }
    if settings.agents.clients.iter().any(|c| c.name == name) {
        return Ok(());
    }
    if settings.agents.clients.len() >= 32 {
        return Err("Too many client profiles.".into());
    }
    settings.agents.clients.push(TrustedClient {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.into(),
        secret: format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ),
        keep_awake: false,
        actions: vec![],
    });
    Ok(())
}
pub fn definitions() -> Value {
    let actions = json!([
        "return_to_normal",
        "sleep",
        "hibernate",
        "lock",
        "display_off",
        "shutdown"
    ]);
    let mut tools = vec![
        json!({"name":"doze.start_session", "description":"Request a wake lease. Awaiting authorization means no wake assertion yet: ask the user to approve in Doze and poll get_session. Heartbeat before lease expiry. Finish only when work is actually complete; disconnect is not completion. Times are monotonic seconds since Doze started.", "inputSchema":{"type":"object", "properties":{"reason":{"type":"string","minLength":1,"maxLength":512}, "completion_action":{"type":"string","enum":actions}, "optional_timeout":{"type":"integer","minimum":30,"maximum":604800}}, "required":["reason"],"additionalProperties":false}, "annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}),
    ];
    for (name, description, readonly) in [
        ("heartbeat", "Renew an authorized lease. Send at least every lease-duration/2 seconds.", false),
        ("finish_session", "Explicitly mark work complete. The core waits for all leases and starts a cancellable countdown; never sleeps directly.", false),
        ("fail_session", "Explicitly report definitive task failure, not disconnection or missing status. Release this authorized lease without executing its action. Successful peer sessions may still complete normally.", false),
        ("cancel_session", "Release your session without executing its completion action.", false),
        ("get_session", "Read your session status and lease expiration.", true),
    ] {
        tools.push(json!({"name":format!("doze.{name}"),"description":description,"inputSchema":{"type":"object","properties":{"session_id":{"type":"string"}},"required":["session_id"],"additionalProperties":false},"annotations":{"readOnlyHint":readonly,"destructiveHint":name == "finish_session","idempotentHint":true,"openWorldHint":false}}));
    }
    tools.push(json!({"name":"doze.list_sessions","description":"List sessions owned by this configured client.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}));
    json!({"tools":tools})
}

fn join_authorized_batch(engine: &mut Engine, id: &str) {
    if !engine.agents.holds_awake() {
        if engine
            .countdown
            .as_ref()
            .is_some_and(|c| c.source == Source::Agents)
        {
            engine.cancel_countdown();
        }
        engine.agents.batch.clear();
        engine.agents.completion_consumed = false;
        prune_history(engine);
    }
    engine.agents.batch.push(id.into());
}

fn prune_history(engine: &mut Engine) {
    let sessions = &mut engine.agents;
    let removable = |s: &Session| s.status.terminal() && !sessions.batch.contains(&s.session_id);
    let mut remove = sessions
        .items
        .iter()
        .filter(|s| removable(s))
        .count()
        .saturating_sub(128);
    sessions.items.retain(|s| {
        if remove > 0 && removable(s) {
            remove -= 1;
            false
        } else {
            true
        }
    });
}

//! Agent sessions from every source — hooks, plugins, MCP leases and detected processes —
//! share the MCP session model in `mcp::sessions`. This module names the agents, derives the
//! state both apps render, and turns native lifecycle events into session updates.
pub mod connect;
pub mod hooks;
pub mod process;
#[cfg(test)]
mod tests;

use crate::mcp::sessions::{Session, Status};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AgentKind {
    ClaudeCode,
    Codex,
    OpenCode,
    GeminiCli,
    Cursor,
    McpClient(String),
    Custom(String),
}

impl AgentKind {
    /// Stable id used in settings, hook commands and the UI.
    pub fn id(&self) -> String {
        match self {
            Self::ClaudeCode => "claude-code".into(),
            Self::Codex => "codex".into(),
            Self::OpenCode => "opencode".into(),
            Self::GeminiCli => "gemini-cli".into(),
            Self::Cursor => "cursor".into(),
            Self::McpClient(name) => format!("mcp:{name}"),
            Self::Custom(name) => format!("custom:{name}"),
        }
    }
    pub fn parse(id: &str) -> Result<Self, String> {
        let invalid =
            |name: &str| name.is_empty() || name.len() > 64 || name.chars().any(|c| c.is_control());
        Ok(match id {
            "claude-code" | "claude" | "claudecode" => Self::ClaudeCode,
            "codex" => Self::Codex,
            "opencode" => Self::OpenCode,
            "gemini-cli" | "gemini" => Self::GeminiCli,
            "cursor" => Self::Cursor,
            _ => match id.split_once(':') {
                Some(("mcp", name)) if !invalid(name) => Self::McpClient(name.into()),
                Some(("custom", name)) if !invalid(name) => Self::Custom(name.into()),
                _ => return Err(format!("Unknown agent \"{id}\".")),
            },
        })
    }
    pub fn label(&self) -> String {
        match self {
            Self::ClaudeCode => "Claude Code".into(),
            Self::Codex => "Codex".into(),
            Self::OpenCode => "OpenCode".into(),
            Self::GeminiCli => "Gemini CLI".into(),
            Self::Cursor => "Cursor".into(),
            Self::McpClient(name) | Self::Custom(name) => name.clone(),
        }
    }
    /// Two letters for the monogram tile.
    pub fn monogram(&self) -> String {
        match self {
            Self::ClaudeCode => "CC".into(),
            Self::Codex => "Cx".into(),
            Self::OpenCode => "OC".into(),
            Self::GeminiCli => "Ge".into(),
            Self::Cursor => "Cu".into(),
            Self::McpClient(name) | Self::Custom(name) => {
                let mut letters = name.chars().filter(|c| c.is_alphanumeric());
                let first = letters.next().map_or('?', |c| c.to_ascii_uppercase());
                letters
                    .next()
                    .map_or(first.to_string(), |second| format!("{first}{second}"))
            }
        }
    }
    /// The MCP client profile names created by Settings › Agents map to known agents.
    pub fn for_client(name: &str) -> Self {
        match name {
            "Claude Code" => Self::ClaudeCode,
            "Codex" => Self::Codex,
            "Command line" => Self::Custom("Command line".into()),
            other => Self::McpClient(other.into()),
        }
    }
}

impl Serialize for AgentKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.id())
    }
}
impl<'de> Deserialize<'de> for AgentKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let id = String::deserialize(deserializer)?;
        Self::parse(&id).map_err(serde::de::Error::custom)
    }
}

/// How Doze learns about a session.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionSource {
    Hooks,
    Plugin,
    #[default]
    McpLease,
    Process,
}

/// What both apps render. Only `Working` holds a power assertion.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentState {
    Working,
    Idle,
    NeedsApproval,
    Done,
}

pub fn state(session: &Session) -> AgentState {
    match session.status {
        Status::AwaitingAuthorization => AgentState::NeedsApproval,
        status if status.terminal() => AgentState::Done,
        _ if session.activity_holds() => AgentState::Working,
        _ => AgentState::Idle,
    }
}

/// The folder name of a working directory, such as "doze-app".
pub fn project(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches(['/', '\\']);
    let name = trimmed.rsplit(['/', '\\']).next()?.trim();
    (!name.is_empty()).then(|| name.chars().take(128).collect())
}

/// A prompt shortened to one line for the session row.
pub fn summary(text: &str) -> Option<String> {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    let mut short: String = line.chars().take(80).collect();
    if line.chars().count() > 80 {
        short.push('…');
    }
    Some(short)
}

/// One row in the panel and Settings. Times are engine seconds; the apps add their own
/// elapsed time between snapshots instead of polling.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub agent: AgentKind,
    pub name: String,
    pub monogram: String,
    pub project: Option<String>,
    pub task: Option<String>,
    pub state: AgentState,
    pub source: SessionSource,
    pub started_at: u64,
    pub last_event_at: u64,
    /// When the current state began.
    pub state_since: u64,
    /// Seconds of earlier Working time, not counting the current stretch.
    pub working_seconds: u64,
    pub holds_assertion: bool,
    pub completion_action: Option<crate::core::sessions::PowerAction>,
    /// Missed check-ins: Doze still holds the lease until this time, then releases it.
    pub releases_at: Option<u64>,
}

pub fn view(session: &Session) -> SessionView {
    let state = state(session);
    SessionView {
        id: session.session_id.clone(),
        agent: session.agent.clone(),
        name: session.agent.label(),
        monogram: session.agent.monogram(),
        project: session.project.clone(),
        task: Some(
            session
                .title
                .clone()
                .unwrap_or_else(|| session.reason.clone()),
        )
        .filter(|task| !task.is_empty()),
        state,
        source: session.source,
        started_at: session.created_at,
        last_event_at: session.last_heartbeat,
        state_since: session.activity_changed_at,
        working_seconds: session.working_seconds,
        holds_assertion: session.holds(),
        completion_action: session.completion_action,
        releases_at: (session.status == Status::ConnectionLost).then(|| {
            session
                .lost_at
                .unwrap_or(session.last_heartbeat)
                .saturating_add(crate::mcp::sessions::LOST_GRACE_SECONDS)
        }),
    }
}

/// Recently ended sessions stay listed (as Done) while their batch can still finish.
pub fn visible(sessions: &crate::mcp::sessions::Sessions) -> Vec<SessionView> {
    sessions
        .items
        .iter()
        .filter(|s| {
            !s.status.terminal()
                || (sessions.batch.contains(&s.session_id)
                    && matches!(s.status, Status::Finished | Status::Failed))
        })
        .map(view)
        .collect()
}

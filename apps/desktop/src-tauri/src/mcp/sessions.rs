use crate::{
    agents::{AgentKind, SessionSource},
    core::sessions::PowerAction,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    AwaitingAuthorization,
    Active,
    ConnectionLost,
    Finished,
    Failed,
    Cancelled,
    Denied,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    #[default]
    Unknown,
    Working,
    /// Waiting on the user, such as a permission prompt. The agent is mid-task, so it keeps
    /// the computer awake.
    Waiting,
    Idle,
}
impl Status {
    pub fn holds_awake(self) -> bool {
        matches!(self, Self::Active | Self::ConnectionLost)
    }
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Finished | Self::Failed | Self::Cancelled | Self::Denied
        )
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Session {
    pub session_id: String,
    pub client_id: String,
    pub client_name: String,
    pub agent: AgentKind,
    pub source: SessionSource,
    /// The project folder name, such as "doze-app".
    pub project: Option<String>,
    pub reason: String,
    pub title: Option<String>,
    pub workspace: Option<String>,
    pub activity: Activity,
    pub parent_session_id: Option<String>,
    pub provider_session_id: Option<String>,
    pub activity_changed_at: u64,
    pub working_seconds: u64,
    pub created_at: u64,
    /// The last event or heartbeat from the agent.
    pub last_heartbeat: u64,
    pub lease_expires_at: u64,
    pub completion_action: Option<PowerAction>,
    pub authorized_action: Option<PowerAction>,
    pub status: Status,
    #[serde(skip)]
    pub timeout_at: Option<u64>,
    /// When the missed check-in grace started: the last event, or a resume from sleep.
    #[serde(skip)]
    pub lost_at: Option<u64>,
    /// Last renewal from the agent or job itself, as opposed to the bridge keep-alive.
    #[serde(skip)]
    pub explicit_at: u64,
    /// The agent asked for its own completion action. Otherwise the When agents finish
    /// setting applies at the moment the batch completes.
    #[serde(skip)]
    pub explicit_action: bool,
}
impl Session {
    /// Idle sessions never hold a power assertion; working and waiting ones do.
    pub fn activity_holds(&self) -> bool {
        self.activity != Activity::Idle
    }
    pub fn holds(&self) -> bool {
        self.status.holds_awake() && self.activity_holds()
    }
    /// Adds the stretch of work that just ended and starts a new activity.
    pub fn set_activity(&mut self, activity: Activity, now: u64) {
        if self.activity == activity {
            return;
        }
        if self.activity_holds() {
            self.working_seconds = self
                .working_seconds
                .saturating_add(now.saturating_sub(self.activity_changed_at));
        }
        self.activity = activity;
        self.activity_changed_at = now;
    }
}
/// Missed check-ins: an agent that stops sending events keeps the computer awake for this
/// long after its last event. The session is then released without any power action, so a
/// crashed agent can never keep the computer awake or defer the user's own timers.
pub const LOST_GRACE_SECONDS: u64 = 1800;
/// While its agent app stays connected, the MCP bridge renews sessions for at most this long
/// after the agent's last own heartbeat. A forgotten finish cannot hold the computer forever.
pub const KEEPALIVE_LIMIT_SECONDS: u64 = 24 * 3600;
/// Idle hook sessions with no event for this long are treated as closed without telling Doze.
pub const IDLE_FORGET_SECONDS: u64 = 8 * 3600;
/// MCP approval requests left unanswered are denied after this long.
pub const PENDING_TIMEOUT_SECONDS: u64 = 600;
/// Command-line jobs started by the user (`doze run`, `doze watch`) use this built-in client.
pub const LOCAL_CLIENT_ID: &str = "command-line";
#[derive(Clone, Debug, Default, Serialize)]
pub struct Sessions {
    pub items: Vec<Session>,
    #[serde(skip)]
    pub batch: Vec<String>,
    #[serde(skip)]
    pub completion_consumed: bool,
}
impl Sessions {
    pub fn holds_awake(&self) -> bool {
        self.items.iter().any(Session::holds)
    }
    pub fn working(&self) -> usize {
        self.items.iter().filter(|s| s.holds()).count()
    }
    pub fn pending(&self) -> usize {
        self.items
            .iter()
            .filter(|s| s.status == Status::AwaitingAuthorization)
            .count()
    }
    pub fn unsettled(&self) -> bool {
        self.items.iter().any(|s| !s.status.terminal())
    }
    /// The next time `expire` would change anything, so the engine can sleep until then.
    pub fn next_deadline(&self) -> Option<u64> {
        self.items
            .iter()
            .filter_map(|s| match s.status {
                Status::Active if s.holds() => {
                    Some(s.lease_expires_at.min(s.timeout_at.unwrap_or(u64::MAX)))
                }
                Status::Active => Some(s.last_heartbeat.saturating_add(IDLE_FORGET_SECONDS)),
                Status::ConnectionLost => Some(
                    s.lost_at
                        .unwrap_or(s.last_heartbeat)
                        .saturating_add(LOST_GRACE_SECONDS),
                ),
                Status::AwaitingAuthorization if s.source == SessionSource::McpLease => {
                    Some(s.created_at.saturating_add(PENDING_TIMEOUT_SECONDS))
                }
                _ => None,
            })
            .min()
    }
    pub fn uncertain(&mut self, now: u64) {
        for session in &mut self.items {
            if session.status == Status::Active && session.holds() {
                session.status = Status::ConnectionLost;
                session.lost_at = Some(now);
            }
        }
    }
    /// Applies missed check-ins and stale requests. Returns the agents whose leases were
    /// released because they stopped checking in.
    pub fn expire(&mut self, now: u64) -> Vec<String> {
        let mut stalled = Vec::new();
        for session in &mut self.items {
            if session.status == Status::Active
                && session.holds()
                && (now >= session.lease_expires_at || session.timeout_at.is_some_and(|t| now >= t))
            {
                session.status = Status::ConnectionLost;
                // The grace counts from the agent's last event, not from the lease expiry.
                session.lost_at = Some(session.last_heartbeat.min(now));
            }
            if session.status == Status::ConnectionLost
                && now.saturating_sub(*session.lost_at.get_or_insert(now)) >= LOST_GRACE_SECONDS
            {
                // Cancellation vetoes the batch: abandonment is never treated as completion.
                session.status = Status::Cancelled;
                stalled.push(session.agent.label());
            }
            if session.status == Status::Active
                && !session.holds()
                && now.saturating_sub(session.last_heartbeat) >= IDLE_FORGET_SECONDS
            {
                session.status = Status::Cancelled;
            }
            if session.status == Status::AwaitingAuthorization
                && session.source == SessionSource::McpLease
                && now.saturating_sub(session.created_at) >= PENDING_TIMEOUT_SECONDS
            {
                session.status = Status::Denied;
            }
        }
        stalled
    }
    /// The action for a settled batch, or none. Only successful outcomes may request an
    /// action: explicit finishes, and hook sessions that went idle after their turn.
    /// Definitive failures release their leases without requesting an action. Cancellation,
    /// missed check-ins, return-to-normal and conflicting requests veto the whole batch.
    /// `default` is the When agents finish setting, used unless an agent asked for its own.
    pub fn completion(&self, default: Option<PowerAction>) -> Option<PowerAction> {
        if self.completion_consumed || self.holds_awake() || self.batch.is_empty() {
            return None;
        }
        let mut action = None;
        for id in &self.batch {
            let session = self.items.iter().find(|s| &s.session_id == id)?;
            let requested = match session.status {
                Status::Failed => continue,
                Status::Finished if session.explicit_action => session.authorized_action?,
                Status::Finished => default?,
                Status::Active
                    if !session.holds()
                        && matches!(
                            session.source,
                            SessionSource::Hooks | SessionSource::Plugin
                        ) =>
                {
                    default?
                }
                _ => return None,
            };
            if action.is_some_and(|a| a != requested) {
                return None;
            }
            action = Some(requested);
        }
        action
    }
}

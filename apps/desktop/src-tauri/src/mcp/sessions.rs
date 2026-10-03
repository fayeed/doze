use crate::core::sessions::PowerAction;
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
    pub reason: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub activity: Activity,
    #[serde(default)]
    pub parent_session_id: Option<String>,
    #[serde(default)]
    pub provider_session_id: Option<String>,
    #[serde(default)]
    pub wake_released: bool,
    #[serde(default)]
    pub activity_changed_at: u64,
    #[serde(default)]
    pub working_seconds: u64,
    pub created_at: u64,
    pub last_heartbeat: u64,
    pub lease_expires_at: u64,
    pub completion_action: Option<PowerAction>,
    pub authorized_action: Option<PowerAction>,
    pub status: Status,
    #[serde(skip)]
    pub timeout_at: Option<u64>,
    #[serde(skip)]
    pub lost_at: Option<u64>,
    /// Last renewal from the agent or job itself, as opposed to the bridge keep-alive.
    #[serde(skip)]
    pub explicit_at: u64,
}
/// A lost connection keeps holding the wake request for this long without a heartbeat.
/// The session is then released without its completion action, so a crashed agent can
/// never keep the computer awake or defer the user's own timers indefinitely.
pub const LOST_GRACE_SECONDS: u64 = 1800;
/// While its agent app stays connected, the MCP bridge renews sessions for at most this long
/// after the agent's last own heartbeat. A forgotten finish cannot hold the computer forever.
pub const KEEPALIVE_LIMIT_SECONDS: u64 = 24 * 3600;
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
        self.items
            .iter()
            .any(|s| s.status.holds_awake() && !s.wake_released)
    }
    pub fn unsettled(&self) -> bool {
        self.items.iter().any(|s| !s.status.terminal())
    }
    pub fn uncertain(&mut self, now: u64) {
        for session in &mut self.items {
            if session.status == Status::Active {
                session.status = Status::ConnectionLost;
                session.lost_at = Some(now);
                session.wake_released = false;
            }
        }
    }
    pub fn expire(&mut self, now: u64) {
        for session in &mut self.items {
            if matches!(session.activity, Activity::Waiting | Activity::Idle)
                && session.status == Status::Active
                && now.saturating_sub(session.activity_changed_at) >= 300
            {
                session.wake_released = true;
            }
            if session.status == Status::Active
                && (now >= session.lease_expires_at || session.timeout_at.is_some_and(|t| now >= t))
            {
                session.status = Status::ConnectionLost;
                session.wake_released = false;
                // Leases never outlive their optional timeout, so this is when contact ended.
                session.lost_at = Some(session.lease_expires_at.min(now));
            }
            if session.status == Status::ConnectionLost
                && now.saturating_sub(*session.lost_at.get_or_insert(now)) >= LOST_GRACE_SECONDS
            {
                // Cancellation vetoes the batch: abandonment is never treated as completion.
                session.status = Status::Cancelled;
            }
            if session.status == Status::AwaitingAuthorization
                && now.saturating_sub(session.created_at) >= 600
            {
                session.status = Status::Denied;
            }
        }
    }
    // Only unanimous successful finishes may request an action; definitive failures
    // release their leases without requesting an action. Cancellation,
    // return-to-normal, and conflicting requests veto the entire overlapping batch.
    pub fn completion(&self) -> Option<PowerAction> {
        if self.completion_consumed || self.holds_awake() || self.batch.is_empty() {
            return None;
        }
        let mut action = None;
        for id in &self.batch {
            let session = self.items.iter().find(|s| &s.session_id == id)?;
            if session.status == Status::Failed {
                continue;
            }
            if session.status != Status::Finished {
                return None;
            }
            let requested = session.authorized_action?;
            if action.is_some_and(|a| a != requested) {
                return None;
            }
            action = Some(requested);
        }
        action
    }
}

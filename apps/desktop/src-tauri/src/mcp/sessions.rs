use crate::core::sessions::PowerAction;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    AwaitingAuthorization,
    Active,
    ConnectionLost,
    Finished,
    Cancelled,
    Denied,
}
impl Status {
    pub fn holds_awake(self) -> bool {
        matches!(self, Self::Active | Self::ConnectionLost)
    }
    pub fn terminal(self) -> bool {
        matches!(self, Self::Finished | Self::Cancelled | Self::Denied)
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Session {
    pub session_id: String,
    pub client_id: String,
    pub client_name: String,
    pub reason: String,
    pub created_at: u64,
    pub last_heartbeat: u64,
    pub lease_expires_at: u64,
    pub completion_action: Option<PowerAction>,
    pub authorized_action: Option<PowerAction>,
    pub status: Status,
    #[serde(skip)]
    pub timeout_at: Option<u64>,
}
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
        self.items.iter().any(|s| s.status.holds_awake())
    }
    pub fn unsettled(&self) -> bool {
        self.items.iter().any(|s| !s.status.terminal())
    }
    pub fn uncertain(&mut self) {
        for session in &mut self.items {
            if session.status.holds_awake() {
                session.status = Status::ConnectionLost;
            }
        }
    }
    pub fn expire(&mut self, now: u64) {
        for session in &mut self.items {
            if session.status == Status::Active
                && (now >= session.lease_expires_at || session.timeout_at.is_some_and(|t| now >= t))
            {
                session.status = Status::ConnectionLost;
            }
            if session.status == Status::AwaitingAuthorization
                && now.saturating_sub(session.created_at) >= 600
            {
                session.status = Status::Denied;
            }
        }
    }
    // Only unanimous explicit finishes may produce an automatic action. Cancellation,
    // denial, return-to-normal, and conflicting requests veto the entire overlapping batch.
    pub fn completion(&self) -> Option<PowerAction> {
        if self.completion_consumed || self.unsettled() || self.batch.is_empty() {
            return None;
        }
        let mut action = None;
        for id in &self.batch {
            let session = self.items.iter().find(|s| &s.session_id == id)?;
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

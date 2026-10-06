use crate::core::sessions::PowerAction;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentSettings {
    pub enabled: bool,
    pub lease_seconds: u64,
    pub default_completion: Option<PowerAction>,
    pub clients: Vec<TrustedClient>,
    /// The MCP bridge renews its sessions while the agent app stays connected, so one long
    /// step without model turns does not lose the lease.
    pub keep_alive_while_connected: bool,
    /// A new agent's first session waits for Allow or Deny before it may hold a lease.
    pub ask_before_new: bool,
    /// Agents (by `AgentKind` id) the user allowed. They stay trusted until removed in Settings.
    pub trusted: Vec<String>,
    /// Tools without lifecycle hooks, detected by their running process.
    pub process_tools: Vec<ProcessTool>,
}
/// A tool detected by process. It shows as Open and never holds the computer awake unless
/// the user turns `keep_awake` on for it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessTool {
    pub id: String,
    pub detect: bool,
    pub keep_awake: bool,
}
impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            lease_seconds: 300,
            default_completion: Some(PowerAction::Sleep),
            clients: vec![],
            keep_alive_while_connected: true,
            ask_before_new: true,
            trusted: vec![],
            process_tools: vec![ProcessTool {
                id: "cursor".into(),
                detect: true,
                keep_awake: false,
            }],
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrustedClient {
    pub id: String,
    pub name: String,
    pub secret: String,
    pub keep_awake: bool,
    pub actions: Vec<PowerAction>,
}
impl AgentSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(30..=3600).contains(&self.lease_seconds) || self.clients.len() > 32 {
            return Err(
                "Agent leases must be 30–3600 seconds; at most 32 clients are supported.".into(),
            );
        }
        let mut ids = std::collections::HashSet::new();
        for client in &self.clients {
            if client.id.is_empty() || client.secret.len() < 32 || !ids.insert(&client.id) {
                return Err("Invalid or duplicate agent credentials.".into());
            }
        }
        if self.trusted.len() > 64
            || self.process_tools.len() > 16
            || self
                .trusted
                .iter()
                .chain(self.process_tools.iter().map(|tool| &tool.id))
                .any(|id| id.is_empty() || id.len() > 128 || id.chars().any(char::is_control))
        {
            return Err("Invalid trusted agents or detected tools.".into());
        }
        Ok(())
    }
    pub fn is_trusted(&self, agent: &str) -> bool {
        !self.ask_before_new || self.trusted.iter().any(|id| id == agent)
    }
    pub fn trust(&mut self, agent: &str) {
        if !self.trusted.iter().any(|id| id == agent) {
            self.trusted.push(agent.into());
        }
    }
    pub fn authenticate(&self, key: &str) -> Result<&TrustedClient, String> {
        if !self.enabled {
            return Err("MCP is disabled. Enable it in Doze’s Agents settings.".into());
        }
        self.clients
            .iter()
            .find(|c| secure_equal(&c.secret, key))
            .ok_or_else(|| {
                "Unknown or revoked agent credential. Connect through Doze’s Agents settings."
                    .into()
            })
    }
}
// Compare the entire credential, independent of the first mismatching byte.
pub fn secure_equal(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

use crate::core::sessions::PowerAction;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentSettings {
    pub enabled: bool,
    pub lease_seconds: u64,
    pub default_completion: Option<PowerAction>,
    pub clients: Vec<TrustedClient>,
}
impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            lease_seconds: 300,
            default_completion: None,
            clients: vec![],
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
        Ok(())
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

use super::{
    auth::TrustedClient,
    sessions::Status,
    tools::{self, Call},
};
use crate::{
    core::{
        countdown::Source,
        sessions::{Engine, PowerAction, Settings},
    },
    platform::PowerManager,
};
use serde_json::{json, Value};
fn settings() -> Settings {
    let mut settings = Settings::default();
    settings.agents.enabled = true;
    settings.agents.lease_seconds = 30;
    for id in ["codex", "claude"] {
        settings.agents.clients.push(TrustedClient {
            id: id.into(),
            name: id.into(),
            secret: format!("{id}-{}", "x".repeat(32)),
            keep_awake: true,
            actions: vec![PowerAction::Sleep],
        });
    }
    settings
}
fn call(
    e: &mut Engine,
    s: &Settings,
    owner: &str,
    name: &str,
    args: Value,
) -> Result<Value, String> {
    tools::call(
        e,
        s,
        &[PowerAction::Sleep, PowerAction::Shutdown, PowerAction::Lock],
        Call {
            key: s
                .agents
                .clients
                .iter()
                .find(|c| c.id == owner)
                .unwrap()
                .secret
                .clone(),
            name: format!("doze.{name}"),
            arguments: args,
        },
    )
}
fn start(e: &mut Engine, s: &Settings, owner: &str, action: &str) -> String {
    call(
        e,
        s,
        owner,
        "start_session",
        json!({"reason":"Run tests", "completion_action":action}),
    )
    .unwrap()["session_id"]
        .as_str()
        .unwrap()
        .into()
}
fn finish(e: &mut Engine, s: &Settings, owner: &str, id: &str) {
    call(e, s, owner, "finish_session", json!({"session_id":id})).unwrap();
}
fn tick(e: &mut Engine, s: &Settings, now: u64) -> Option<PowerAction> {
    e.tick(now, None, None, false, s)
}
#[derive(Default)]
struct MockPower {
    awake: bool,
    actions: Vec<PowerAction>,
}
impl PowerManager for MockPower {
    fn supported_actions(&self) -> Vec<PowerAction> {
        vec![PowerAction::Sleep]
    }
    fn set_awake(&mut self, active: bool, _: bool) -> Result<(), String> {
        self.awake = active;
        Ok(())
    }
    fn execute(&mut self, action: PowerAction) -> Result<(), String> {
        self.actions.push(action);
        Ok(())
    }
}
#[test]
fn start_acquires_mock_assertion_and_finish_only_starts_core_countdown() {
    let (mut e, s, mut power) = (Engine::default(), settings(), MockPower::default());
    let id = start(&mut e, &s, "codex", "sleep");
    power.set_awake(e.should_hold_awake(), false).unwrap();
    assert!(power.awake);
    assert_eq!(e.agents.items[0].lease_expires_at, 30);
    finish(&mut e, &s, "codex", &id);
    assert!(power.actions.is_empty());
    assert_eq!(tick(&mut e, &s, 1), None);
    assert_eq!(e.countdown.as_ref().unwrap().source, Source::Agents);
    assert_eq!(e.countdown.as_ref().unwrap().deadline, 301);
    if let Some(action) = tick(&mut e, &s, 301) {
        power.execute(action).unwrap();
    }
    assert_eq!(power.actions, vec![PowerAction::Sleep]);
    assert_eq!(tick(&mut e, &s, 302), None);
}
#[test]
fn heartbeat_renews_and_recovers_lost_lease() {
    let (mut e, s) = (Engine::default(), settings());
    let id = start(&mut e, &s, "codex", "sleep");
    tick(&mut e, &s, 30);
    assert_eq!(e.agents.items[0].status, Status::ConnectionLost);
    call(&mut e, &s, "codex", "heartbeat", json!({"session_id":id})).unwrap();
    assert_eq!(e.agents.items[0].status, Status::Active);
    assert_eq!(e.agents.items[0].last_heartbeat, 30);
    assert_eq!(e.agents.items[0].lease_expires_at, 60);
}
#[test]
fn disconnect_or_expiry_never_schedules_a_power_action() {
    let (mut e, s) = (Engine::default(), settings());
    start(&mut e, &s, "codex", "sleep");
    for now in [30, 300, 3600, 86400] {
        assert_eq!(tick(&mut e, &s, now), None);
        assert!(e.should_hold_awake());
        assert!(e.countdown.is_none());
    }
}
#[test]
fn other_agents_block_completion_until_last_explicit_finish() {
    let (mut e, s) = (Engine::default(), settings());
    let a = start(&mut e, &s, "codex", "sleep");
    let b = start(&mut e, &s, "claude", "sleep");
    finish(&mut e, &s, "codex", &a);
    tick(&mut e, &s, 1);
    assert!(e.countdown.is_none());
    assert!(e.should_hold_awake());
    finish(&mut e, &s, "claude", &b);
    tick(&mut e, &s, 2);
    assert!(e.countdown.is_some());
}
#[test]
fn lost_peer_blocks_finished_peer() {
    let (mut e, s) = (Engine::default(), settings());
    let a = start(&mut e, &s, "codex", "sleep");
    start(&mut e, &s, "claude", "sleep");
    finish(&mut e, &s, "codex", &a);
    tick(&mut e, &s, 1000);
    assert!(e.countdown.is_none());
    assert!(e.should_hold_awake());
}
#[test]
fn conflicting_actions_and_return_to_normal_veto_batch() {
    for action in ["return_to_normal", "lock"] {
        let (mut e, mut s) = (Engine::default(), settings());
        s.agents.clients[1].actions.push(PowerAction::Lock);
        let a = start(&mut e, &s, "codex", "sleep");
        let b = start(&mut e, &s, "claude", action);
        finish(&mut e, &s, "codex", &a);
        finish(&mut e, &s, "claude", &b);
        tick(&mut e, &s, 1000);
        assert!(e.countdown.is_none());
        assert!(!e.should_hold_awake());
    }
}
#[test]
fn cancelled_peer_vetoes_completion_action() {
    let (mut e, s) = (Engine::default(), settings());
    let a = start(&mut e, &s, "codex", "sleep");
    let b = start(&mut e, &s, "claude", "sleep");
    call(
        &mut e,
        &s,
        "claude",
        "cancel_session",
        json!({"session_id":b}),
    )
    .unwrap();
    finish(&mut e, &s, "codex", &a);
    tick(&mut e, &s, 1);
    assert!(e.countdown.is_none());
}
#[test]
fn countdown_cancel_does_not_rearm() {
    let (mut e, s) = (Engine::default(), settings());
    let a = start(&mut e, &s, "codex", "sleep");
    finish(&mut e, &s, "codex", &a);
    tick(&mut e, &s, 1);
    e.cancel_countdown();
    tick(&mut e, &s, 1000);
    assert!(e.countdown.is_none());
}
#[test]
fn manual_wake_defers_agent_completion() {
    let (mut e, s) = (Engine::default(), settings());
    e.keep_awake(None);
    let id = start(&mut e, &s, "codex", "sleep");
    finish(&mut e, &s, "codex", &id);
    tick(&mut e, &s, 1);
    assert!(e.countdown.is_none());
    e.stop_awake();
    tick(&mut e, &s, 2);
    assert!(e.countdown.is_some());
}
#[test]
fn timer_and_existing_countdown_respect_agent_leases() {
    let (mut e, s) = (Engine::default(), settings());
    e.schedule(1, PowerAction::Shutdown);
    start(&mut e, &s, "codex", "sleep");
    assert_eq!(tick(&mut e, &s, 1), None);
    assert_eq!(tick(&mut e, &s, 1000), None);
    assert!(e.should_hold_awake());
}
#[test]
fn fresh_session_cancels_prior_agent_countdown() {
    let (mut e, s) = (Engine::default(), settings());
    let a = start(&mut e, &s, "codex", "sleep");
    finish(&mut e, &s, "codex", &a);
    tick(&mut e, &s, 1);
    start(&mut e, &s, "claude", "sleep");
    assert!(e.countdown.is_none());
    assert!(e.should_hold_awake());
}
#[test]
fn unapproved_shutdown_cannot_finish_or_hold_awake() {
    let (mut e, s) = (Engine::default(), settings());
    let id = start(&mut e, &s, "codex", "shutdown");
    assert_eq!(e.agents.items[0].status, Status::AwaitingAuthorization);
    assert!(!e.should_hold_awake());
    assert!(call(
        &mut e,
        &s,
        "codex",
        "finish_session",
        json!({"session_id":id})
    )
    .is_err());
    assert!(call(&mut e, &s, "codex", "heartbeat", json!({"session_id":id})).is_err());
}
#[test]
fn allow_once_does_not_persist_but_always_allow_does() {
    let (mut e, mut s) = (Engine::default(), settings());
    let a = start(&mut e, &s, "codex", "shutdown");
    tools::authorize(&mut e, &mut s, &a, "once").unwrap();
    assert!(!s.agents.clients[0].actions.contains(&PowerAction::Shutdown));
    let b = start(&mut e, &s, "codex", "shutdown");
    tools::authorize(&mut e, &mut s, &b, "always").unwrap();
    let restored: Settings = serde_json::from_value(json!(s)).unwrap();
    assert!(restored.agents.clients[0]
        .actions
        .contains(&PowerAction::Shutdown));
}
#[test]
fn deny_and_pending_timeout_never_hold_awake() {
    let (mut e, mut s) = (Engine::default(), settings());
    let a = start(&mut e, &s, "codex", "shutdown");
    tools::authorize(&mut e, &mut s, &a, "deny").unwrap();
    let b = start(&mut e, &s, "codex", "shutdown");
    tick(&mut e, &s, 600);
    assert!(tools::authorize(&mut e, &mut s, &b, "once").is_err());
    assert!(!e.should_hold_awake());
}
#[test]
fn disabled_unknown_credentials_and_cross_client_access_are_rejected() {
    let (mut e, mut s) = (Engine::default(), settings());
    let a = start(&mut e, &s, "codex", "sleep");
    for name in [
        "get_session",
        "heartbeat",
        "finish_session",
        "cancel_session",
    ] {
        assert!(call(&mut e, &s, "claude", name, json!({"session_id":a})).is_err());
    }
    assert!(tools::call(
        &mut e,
        &s,
        &[],
        Call {
            key: "invalid".into(),
            name: "doze.list_sessions".into(),
            arguments: json!({})
        }
    )
    .is_err());
    s.agents.enabled = false;
    assert!(call(&mut e, &s, "codex", "get_session", json!({"session_id":a})).is_err());
}
#[test]
fn validation_rejects_empty_oversized_unknown_and_out_of_bounds_arguments() {
    let (mut e, s) = (Engine::default(), settings());
    for args in [
        json!({"reason":""}),
        json!({"reason":"x".repeat(513)}),
        json!({"reason":"x","execute":"cmd"}),
        json!({"reason":"x","optional_timeout":0}),
        json!({"reason":"x","completion_action":"sleep_now"}),
    ] {
        assert!(call(&mut e, &s, "codex", "start_session", args).is_err());
    }
    assert!(call(&mut e, &s, "codex", "shutdown_now", json!({})).is_err());
}
#[test]
fn suspend_and_clock_reset_preserve_uncertain_wake_without_completion() {
    let (mut e, s) = (Engine::default(), settings());
    start(&mut e, &s, "codex", "sleep");
    e.reset_transient("resume");
    assert_eq!(e.agents.items[0].status, Status::ConnectionLost);
    assert_eq!(tick(&mut e, &s, 1000), None);
    assert!(e.should_hold_awake());
}
#[test]
fn optional_timeout_is_uncertainty_and_cannot_be_renewed() {
    let (mut e, s) = (Engine::default(), settings());
    let id = call(
        &mut e,
        &s,
        "codex",
        "start_session",
        json!({"reason":"test","optional_timeout":30}),
    )
    .unwrap()["session_id"]
        .clone();
    tick(&mut e, &s, 30);
    assert!(call(&mut e, &s, "codex", "heartbeat", json!({"session_id":id})).is_err());
    assert!(e.should_hold_awake());
}
#[test]
fn session_listing_is_scoped_and_finished_session_remains_queryable() {
    let (mut e, s) = (Engine::default(), settings());
    let id = start(&mut e, &s, "codex", "sleep");
    finish(&mut e, &s, "codex", &id);
    start(&mut e, &s, "claude", "sleep");
    let list = call(&mut e, &s, "codex", "list_sessions", json!({})).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(
        call(&mut e, &s, "codex", "get_session", json!({"session_id":id})).unwrap()["status"],
        "finished"
    );
}
#[test]
fn json_rpc_lifecycle_and_tool_errors_are_compliant() {
    let mut initialized = false;
    let mut invoke = |_, _| Err("not authorized".into());
    let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/list"});
    assert!(
        super::server::dispatch(&request, &mut initialized, false, &mut invoke)["error"]
            .is_object()
    );
    let init = json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"test","version":"1"},"capabilities":{}}});
    assert_eq!(
        super::server::dispatch(&init, &mut initialized, false, &mut invoke)["result"]
            ["protocolVersion"],
        "2025-11-25"
    );
    assert_eq!(
        super::server::dispatch(&request, &mut initialized, true, &mut invoke)["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    let tool = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"doze.start_session","arguments":{"reason":"test"}}});
    assert_eq!(
        super::server::dispatch(&tool, &mut initialized, true, &mut invoke)["result"]["isError"],
        true
    );
}

#[test]
fn connection_configuration_uses_documented_schemas_and_escaped_paths() {
    let settings = settings();
    let path = std::path::Path::new("C:\\Users\\Name With Spaces\\settings.json");
    let configs = super::server::connection_configs(&settings, path);
    let generic: Value = serde_json::from_str(configs[0]["generic"].as_str().unwrap()).unwrap();
    let claude: Value = serde_json::from_str(configs[0]["claude"].as_str().unwrap()).unwrap();
    assert_eq!(claude["type"], "stdio");
    assert_eq!(generic["mcpServers"]["doze"], claude);
    assert_eq!(
        claude["env"]["DOZE_MCP_KEY"],
        settings.agents.clients[0].secret
    );
    assert!(configs[0]["codex"]
        .as_str()
        .unwrap()
        .contains("[mcp_servers.doze.env]"));
}
#[test]
fn existing_timer_countdown_pauses_without_repeated_warnings() {
    let (mut engine, settings) = (Engine::default(), settings());
    engine.schedule(1, PowerAction::Sleep);
    tick(&mut engine, &settings, 1);
    assert!(engine.countdown.is_some());
    let id = start(&mut engine, &settings, "codex", "sleep");
    tick(&mut engine, &settings, 2);
    assert!(engine.countdown.is_none());
    assert!(engine.timer.is_some());
    tick(&mut engine, &settings, 1000);
    assert!(engine.countdown.is_none());
    finish(&mut engine, &settings, "codex", &id);
    tick(&mut engine, &settings, 1001);
    assert_eq!(engine.countdown.as_ref().unwrap().source, Source::Timer);
    assert_eq!(engine.countdown.as_ref().unwrap().deadline, 1301);
}

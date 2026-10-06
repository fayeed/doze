//! The agent session state machine, driven by the engine's injected clock (`tick(now, …)`).
use super::{hooks, process, state, AgentKind, AgentState};
use crate::{
    core::{
        countdown::Source,
        rules::AGENT_FINISH_IDLE_SECONDS,
        sessions::{Engine, Event, PowerAction, Settings},
    },
    mcp::{sessions::Status, tools},
};
use serde_json::json;

fn trusted() -> Settings {
    let mut settings = Settings::default();
    settings.agents.trusted = vec!["claude-code".into(), "codex".into()];
    settings
}
fn hook(e: &mut Engine, s: &Settings, now: u64, agent: &str, event: &str, id: &str) {
    e.now = now;
    hooks::call(
        e,
        s,
        "event",
        json!({"agent": agent, "event": event, "session_id": id, "cwd": "/Users/me/dev/doze-app", "prompt": "Run the test suite\nand fix failures"}),
    )
    .unwrap();
}
/// A tick with the user away from the keyboard.
fn away(e: &mut Engine, s: &Settings, now: u64) -> Option<PowerAction> {
    e.tick(now, None, Some(AGENT_FINISH_IDLE_SECONDS), false, s)
}
fn only(e: &Engine) -> &crate::mcp::sessions::Session {
    assert_eq!(e.agents.items.len(), 1);
    &e.agents.items[0]
}

#[test]
fn prompt_works_stop_idles_and_releases_the_assertion() {
    let (mut e, s) = (Engine::default(), trusted());
    hook(&mut e, &s, 0, "claude-code", "SessionStart", "a");
    assert_eq!(state(only(&e)), AgentState::Idle);
    assert!(!e.should_hold_awake());
    hook(&mut e, &s, 10, "claude-code", "UserPromptSubmit", "a");
    let session = only(&e);
    assert_eq!(state(session), AgentState::Working);
    assert_eq!(session.project.as_deref(), Some("doze-app"));
    assert_eq!(session.reason, "Run the test suite");
    assert!(e.should_hold_awake());
    // Waiting on the user mid-task still keeps the machine awake.
    hook(&mut e, &s, 20, "claude-code", "Notification", "a");
    assert_eq!(state(only(&e)), AgentState::Working);
    assert!(e.should_hold_awake());
    hook(&mut e, &s, 70, "claude-code", "Stop", "a");
    assert_eq!(state(only(&e)), AgentState::Idle);
    assert_eq!(only(&e).working_seconds, 60);
    assert!(!e.should_hold_awake());
    hook(&mut e, &s, 80, "claude-code", "SessionEnd", "a");
    assert_eq!(state(only(&e)), AgentState::Done);
}

#[test]
fn approval_flow_allow_trusts_the_agent_and_deny_ends_the_session() {
    let (mut e, mut s) = (Engine::default(), Settings::default());
    assert!(s.agents.ask_before_new);
    // A brand-new agent's first session asks and holds nothing.
    hook(&mut e, &s, 0, "codex", "UserPromptSubmit", "a");
    assert_eq!(state(only(&e)), AgentState::NeedsApproval);
    assert!(!e.should_hold_awake());
    assert!(matches!(
        e.events.as_slice(),
        [Event::ApprovalNeeded { agent, .. }] if agent == "Codex"
    ));
    // Further events while asking neither hold nor ask again.
    hook(&mut e, &s, 5, "codex", "PostToolUse", "a");
    assert_eq!(e.agents.pending(), 1);
    assert!(!e.should_hold_awake());
    // Allow: the waiting session works at once and Codex is trusted from now on.
    let id = e.agents.items[0].session_id.clone();
    tools::authorize(&mut e, &mut s, &id, "allow").unwrap();
    assert_eq!(state(only(&e)), AgentState::Working);
    assert!(e.should_hold_awake());
    assert_eq!(s.agents.trusted, vec!["codex".to_string()]);
    hook(&mut e, &s, 10, "codex", "UserPromptSubmit", "b");
    assert_eq!(e.agents.pending(), 0);
    // Deny: the session ends without holding, and later events of that session stay ignored.
    hook(&mut e, &s, 20, "gemini-cli", "BeforeAgent", "g");
    let denied = e.agents.items.last().unwrap().session_id.clone();
    tools::authorize(&mut e, &mut s, &denied, "deny").unwrap();
    hook(&mut e, &s, 21, "gemini-cli", "AfterTool", "g");
    assert_eq!(e.agents.pending(), 0);
    assert!(!s.agents.trusted.contains(&"gemini-cli".to_string()));
    assert_eq!(
        e.agents
            .items
            .iter()
            .filter(|session| session.client_id == "gemini-cli")
            .map(|session| session.status)
            .collect::<Vec<_>>(),
        vec![Status::Denied]
    );
    // Removing the agent in Settings asks again next time.
    s.agents.trusted.clear();
    hook(&mut e, &s, 30, "codex", "UserPromptSubmit", "c");
    assert_eq!(e.agents.pending(), 1);
    // Turning the question off trusts every agent.
    s.agents.ask_before_new = false;
    hook(&mut e, &s, 31, "opencode", "chat.message", "o");
    assert_eq!(state(e.agents.items.last().unwrap()), AgentState::Working);
}

#[test]
fn missed_check_ins_hold_thirty_minutes_after_the_last_event_then_release_without_action() {
    let (mut e, s) = (Engine::default(), trusted());
    hook(&mut e, &s, 100, "claude-code", "UserPromptSubmit", "a");
    hook(&mut e, &s, 400, "claude-code", "PostToolUse", "a");
    // The agent is killed mid-task: no more events.
    for now in [401, 1000, 2199] {
        assert_eq!(away(&mut e, &s, now), None);
        assert!(e.should_hold_awake(), "still within 30 minutes at {now}");
    }
    assert_eq!(away(&mut e, &s, 2200), None);
    assert!(!e.should_hold_awake());
    assert_eq!(only(&e).status, Status::Cancelled);
    assert!(e.events.contains(&Event::AgentStalled {
        agent: "Claude Code".into()
    }));
    // No final warning and no action, ever.
    for now in [2201, 5000, 90000] {
        assert_eq!(away(&mut e, &s, now), None);
        assert!(e.countdown.is_none());
    }
}

#[test]
fn an_event_during_the_missed_check_in_grace_resumes_the_session() {
    let (mut e, s) = (Engine::default(), trusted());
    hook(&mut e, &s, 0, "claude-code", "UserPromptSubmit", "a");
    e.reset_transient("System resumed");
    assert_eq!(only(&e).status, Status::ConnectionLost);
    hook(&mut e, &s, 600, "claude-code", "PostToolUse", "a");
    assert_eq!(only(&e).status, Status::Active);
    away(&mut e, &s, 2399);
    assert!(e.should_hold_awake());
}

#[test]
fn last_working_session_ending_starts_the_final_warning_then_the_action() {
    let (mut e, mut s) = (Engine::default(), trusted());
    s.agents.default_completion = Some(PowerAction::Hibernate);
    hook(&mut e, &s, 0, "claude-code", "UserPromptSubmit", "a");
    hook(&mut e, &s, 0, "codex", "UserPromptSubmit", "b");
    hook(&mut e, &s, 50, "claude-code", "Stop", "a");
    // Another agent is still working.
    assert_eq!(away(&mut e, &s, 300), None);
    assert!(e.countdown.is_none());
    assert!(e.should_hold_awake());
    hook(&mut e, &s, 400, "codex", "Stop", "b");
    // The user is at the keyboard: wait for them to step away, never act straight away.
    assert_eq!(e.tick(401, None, Some(5), false, &s), None);
    assert!(e.countdown.is_none());
    assert!(e.completion_waiting(&s));
    assert_eq!(away(&mut e, &s, 520), None);
    let countdown = e.countdown.clone().unwrap();
    assert_eq!(countdown.source, Source::Agents);
    assert_eq!(countdown.action, PowerAction::Hibernate);
    assert_eq!(countdown.deadline, 520 + 300);
    assert!(e.events.contains(&Event::AgentsFinished {
        action: PowerAction::Hibernate
    }));
    // The final warning always runs to its deadline first.
    assert_eq!(away(&mut e, &s, 819), None);
    assert_eq!(away(&mut e, &s, 820), Some(PowerAction::Hibernate));
    assert_eq!(away(&mut e, &s, 821), None);
}

#[test]
fn new_work_cancels_the_agents_warning_and_nothing_means_no_action() {
    let (mut e, mut s) = (Engine::default(), trusted());
    hook(&mut e, &s, 0, "claude-code", "UserPromptSubmit", "a");
    hook(&mut e, &s, 10, "claude-code", "Stop", "a");
    away(&mut e, &s, 200);
    assert!(e.countdown.is_some());
    hook(&mut e, &s, 210, "claude-code", "UserPromptSubmit", "a");
    assert!(e.countdown.is_none());
    assert!(e.should_hold_awake());
    // When agents finish: Nothing.
    s.agents.default_completion = None;
    hook(&mut e, &s, 220, "claude-code", "Stop", "a");
    assert_eq!(away(&mut e, &s, 1000), None);
    assert!(e.countdown.is_none());
}

#[test]
fn battery_guard_releases_agents_and_keep_awake_once_and_notifies() {
    let (mut e, s) = (Engine::default(), trusted());
    e.keep_awake(None);
    hook(&mut e, &s, 0, "claude-code", "UserPromptSubmit", "a");
    e.schedule(3600, PowerAction::Sleep);
    // Plugged in, or above the floor: nothing happens.
    e.battery(10, false, 15);
    e.battery(40, true, 15);
    assert!(e.awake && e.agents.holds_awake());
    e.battery(14, true, 15);
    assert!(!e.awake);
    assert!(!e.agents.holds_awake());
    assert!(e.timer.is_some(), "the user's own timer stays");
    assert_eq!(
        e.events
            .iter()
            .filter(|event| matches!(event, Event::BatteryGuard { percent: 14 }))
            .count(),
        1
    );
    // While low, new agent work does not hold the computer, and no second notification.
    hook(&mut e, &s, 10, "codex", "UserPromptSubmit", "b");
    away(&mut e, &s, 11);
    assert!(!e.agents.holds_awake());
    e.battery(13, true, 15);
    assert_eq!(
        e.events
            .iter()
            .filter(|event| matches!(event, Event::BatteryGuard { .. }))
            .count(),
        1
    );
    // Released leases never count as finished work.
    assert_eq!(away(&mut e, &s, 2000), None);
    assert!(e.countdown.is_none());
    // Power returns: agents may hold again. A floor of 0 turns the guard off.
    e.battery(13, false, 15);
    hook(&mut e, &s, 2100, "codex", "UserPromptSubmit", "c");
    assert!(e.agents.holds_awake());
    e.battery(5, true, 0);
    assert!(e.agents.holds_awake());
}

#[test]
fn detected_processes_show_as_open_and_hold_only_when_turned_on() {
    let (mut e, mut s) = (Engine::default(), Settings::default());
    assert_eq!(
        process::detect(&["Cursor.exe".into(), "x".into()]),
        vec!["cursor"]
    );
    process::observe(&mut e, &s, &["cursor"]);
    let session = only(&e);
    assert_eq!(session.agent, AgentKind::Cursor);
    assert_eq!(state(session), AgentState::Idle);
    assert!(!e.should_hold_awake());
    s.agents.process_tools[0].keep_awake = true;
    process::observe(&mut e, &s, &["cursor"]);
    assert!(e.should_hold_awake());
    // Quitting the tool releases it without any power action.
    process::observe(&mut e, &s, &[]);
    assert!(!e.should_hold_awake());
    assert_eq!(away(&mut e, &s, 1000), None);
    assert!(e.countdown.is_none());
}

#[test]
fn every_tool_maps_onto_the_same_lifecycle() {
    use hooks::{classify, Lifecycle::*};
    let cases = [
        (AgentKind::Codex, "UserPromptSubmit", Prompt),
        (AgentKind::Codex, "agent-turn-complete", Idle),
        (AgentKind::GeminiCli, "BeforeAgent", Prompt),
        (AgentKind::GeminiCli, "AfterAgent", Idle),
        (AgentKind::GeminiCli, "Notification", Waiting),
        (AgentKind::OpenCode, "chat.message", Prompt),
        (AgentKind::OpenCode, "session.status:busy", Prompt),
        (AgentKind::OpenCode, "session.idle", Idle),
        (AgentKind::OpenCode, "session.deleted", End),
        (AgentKind::Custom("build".into()), "working", Prompt),
        (AgentKind::ClaudeCode, "SessionEnd", End),
    ];
    for (agent, event, expected) in cases {
        assert_eq!(classify(&agent, event), Some(expected), "{agent:?} {event}");
    }
    assert_eq!(classify(&AgentKind::ClaudeCode, "SomethingNew"), None);
    let mut e = Engine::default();
    let s = trusted();
    // Unknown events are ignored rather than failing the agent's hook.
    assert!(hooks::call(
        &mut e,
        &s,
        "event",
        json!({"agent":"claude-code","event":"New"})
    )
    .is_ok());
    assert!(hooks::call(
        &mut e,
        &s,
        "event",
        json!({"agent":"cursor","event":"working"})
    )
    .is_err());
    let mut off = trusted();
    off.agents.enabled = false;
    assert!(hooks::call(
        &mut e,
        &off,
        "event",
        json!({"agent":"codex","event":"Stop"})
    )
    .is_err());
}

#[test]
fn agent_ids_round_trip_and_name_their_tiles() {
    for id in [
        "claude-code",
        "codex",
        "opencode",
        "gemini-cli",
        "cursor",
        "mcp:Zed",
        "custom:ffmpeg",
    ] {
        assert_eq!(AgentKind::parse(id).unwrap().id(), id);
    }
    assert_eq!(AgentKind::OpenCode.monogram(), "OC");
    assert_eq!(AgentKind::McpClient("zed editor".into()).monogram(), "Ze");
    assert!(AgentKind::parse("mcp:").is_err());
    assert_eq!(super::project("C:\\work\\api\\"), Some("api".into()));
}

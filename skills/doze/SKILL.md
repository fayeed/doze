---
name: doze
description: Use Doze MCP when the user asks to keep this computer awake during agent work or sleep, hibernate, lock, turn off the display, or shut down after that work finishes. Also use to inspect or stop the user's Doze agent session.
---

# Doze

Manage an explicit wake session through the local Doze desktop app. Loading this skill does not authorize a session or a power action. Ordinary coding work does not need Doze unless the user requests it.

## Start and authorize

- Discover the connected Doze MCP tools. Logical names below may have a client-specific prefix. If unavailable, explain that the user must run Doze, enable MCP in Agents settings, and connect this client. Do not substitute shell power commands or change client credentials/settings.
- Call `doze.start_session` with a short `reason` describing the authorized work and an explicit `completion_action`. Use `return_to_normal` for keeping awake only. Use `sleep`, `hibernate`, `lock`, `display_off`, or `shutdown` only when the user requests that exact outcome. Ask if the requested outcome is ambiguous; never infer Shutdown from Sleep.
- Supply `optional_timeout` only when requested. This is a total duration in seconds (30–604800), not a completion deadline: expiry means uncertainty and does not trigger the action.
- Retain the returned `session_id` for this job. If `status` is `awaiting_authorization`, tell the user to choose Allow Once or Deny in Doze, then use `doze.get_session` to check the decision. Pending sessions do not keep the computer awake. Denied requests must not be retried to bypass the decision. Persistent permissions belong in Doze Settings.
- Claim wake protection only after the response confirms `active`. Doze must remain running; sessions do not survive an app restart.

## Renew while work continues

Send `doze.heartbeat` with the session ID at least twice per lease interval. Calculate the interval from `lease_expires_at - last_heartbeat`; these values are monotonic seconds since Doze started, not Unix timestamps. The default lease is 300 seconds. Heartbeats do not extend the optional total timeout.

Prefer a runtime integration that renews from fresh authoritative job status without invoking the model. Its status must describe the whole job and its child tasks, not a cached running flag or merely a live MCP connection. This skill installs no runtime hooks or background renewer. If automatic renewal is unavailable, renew at work checkpoints within the interval and disclose that a long uninterrupted operation can lose contact. Do not claim that skill instructions guarantee renewal after a crash, a long model turn, or user Stop.

Missing heartbeats or disconnection means `connection_lost`, not success or definitive failure. Doze keeps awake for up to 30 minutes, until the user resolves it or a heartbeat recovers it; after that the session is cancelled without its completion action. Use `get_session` or `list_sessions` to inspect sessions belonging to this client. Recover a known session with a heartbeat only if its job is still running and Doze accepts renewal; do not invent replacement sessions to bypass a timeout or revocation.

## Report the actual outcome

| Observed outcome | Tool | Effect |
| --- | --- | --- |
| All requested work and required verification succeeded | `doze.finish_session` | Releases this lease; may start the authorized countdown after other work and rules permit it. |
| Definitive terminal job failure, such as an unrecoverable model error | `doze.fail_session` | Releases this lease without requesting its power action; successful peers may still complete normally. |
| User stops or cancels this job | `doze.cancel_session` | Releases this session and vetoes automatic completion for its overlapping batch. |
| Missing status, disconnected runtime, or uncertain outcome | No terminal outcome call | Stop renewal; leave the session uncertain and explain how to resolve it in Doze. |

Pass `session_id` to each tool. A recoverable error while work continues is not terminal failure. An individual turn ending or asking for approval is not whole-job success. If the user changes the requested completion action, cancel the old session and start a newly authorized one.

A stopped model may have no opportunity to call `cancel_session`. The runtime must report authoritative user cancellation; otherwise lease expiry remains the fallback. Never disguise cancellation or lost contact as success to release a blocked countdown.

## Multiple agents and existing rules

Track one session for the job you own. Finish it only after all required child work resolves; do not create child agents merely to use Doze. Independently tracked jobs can have their own sessions, and each owner reports its outcome. Do not mark another job finished because it is quiet or assume cross-client access.

Confirmed failed peers release their leases. A lost peer continues blocking completion. Successful sessions must agree on their authorized action; cancellation, Return to Normal, or conflicting actions veto automatic completion for the batch. A batch of only failures never requests an agent power action.

Doze arbitrates existing timers, manual wake/audio sessions, and After Playback rules. Do not cancel or replace them unless the user requests it. `finish_session` does not sleep immediately: Doze uses a cancellable countdown of at least five minutes, with Cancel, Stay Awake, and Snooze. Report the returned session state and distinguish completed work from an OS action that has actually occurred.

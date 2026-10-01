# Windows QA — 1 October 2026

This run found and fixed a countdown command race. Engine updates previously
re-enabled buttons before acknowledgement, cleared command errors every second,
and ignored errors from Stay Awake. Regression checks reproduced the race before
the fix and pass afterward. The fix keeps commands disabled until acknowledgement
and preserves errors during ordinary countdown updates.

Additional error-state renders exposed a clipped action label when an InfoBar
occupied the fixed-height warning. The countdown now expands while the error is
open and returns to its compact size when closed. Real and error countdown states
are included in visual exports, with a layout check for the action label.

A constrained 360-DIP height reproduced the same clipping on a small display at
high scaling. The warning now fits the monitor work area and scrolls its content
when necessary. Layout checks verify both the action label and access to the
controls; constrained light/dark renders passed after failing before the fix.

## Verified

| Area | Evidence |
| --- | --- |
| Engine, timers, playback, settings, agents | 84 Rust tests passed; the one hardware probe is excluded from the default suite |
| Runtime lifecycle | Six Node tests passed: renewal, success, failure, uncertainty, hung status, authorization wait, abort |
| Actual stdio transport | Official MCP SDK integration passed against the real executable and MockPower engine host |
| Native UI | Nine pages in both themes, four duration/date forms, bounds, edit retention/rollback, approval and connection controls, preview safety, countdown commands and new race/error regressions passed |
| Visual review | 36 rendered artifacts, including every settings page and preview/real/error/constrained countdown states in both themes; verification exports use an opaque substitute for Mica so navigation is readable |
| Hardware probe | Windows supports Sleep, Hibernate, Lock, Display Off and Shutdown; idle detection worked; meaningful current audio was detected |
| Live agent | Existing Codex profile required Allow Once; Return to Normal session became active, renewed its 300-second lease, and explicitly finished successfully. A separate Sleep session required another Allow Once, renewed, and finished; its native countdown window appeared |
| Actual Sleep/recovery | Windows Kernel-Power event 42 records Application API sleep. Power-Troubleshooter event 1 records Sleep at 09:28:41.596 IST and Wake at 13:21:20.073 IST. Doze's original desktop process remained running and the finished MCP session was queryable after wake |
| Live cancellation/cleanup | A new pending Return to Normal session was cancelled before authorization, with no power action; the revised QA runner exited cleanly |
| Build | Windows x64 NSIS installer rebuilt with the constrained-display fix, approximately 59.27 MiB |
| Installed app | Approved NSIS installation completed at `%LOCALAPPDATA%\Doze`; installed engine and native Settings companion launched. A second launch opened Settings. Installed MCP transport created and cancelled a pending session successfully |
| Installed upgrade | Computer Use completed the existing-version Add/Reinstall flow. Installed `Doze.Settings.dll` SHA-256 matches the verified publish output; upgraded MCP connection, status query, and cancellation passed |
| Installed startup mode | Launching installed `doze.exe --startup` exposed no Doze window while the process and MCP transport remained available; its test session was cancelled. This tests the launch mode, not an actual Windows sign-in |
| Bounded idle resources | Over 30 seconds, installed engine CPU increased by 0 seconds and private memory stayed at 3,301,376 bytes. Settings CPU increased by 0.0781 seconds and private memory decreased by 163,840 bytes to 125,071,360. Both processes reported responding. This is a short idle sample, not a long-running leak test |
| Checks | Full workspace lint and tests passed using an isolated Cargo target directory; desktop formatting and diff whitespace checks passed |

The MCP integration exercises concurrent clients, action conflicts, unauthorized
actions, cross-client ownership, cancellation, definitive failure, disconnect,
expiry/recovery, and large bounded session history. These power actions use mocks.
The hardware probe performs no power action.

## Still requiring verification

- Computer Use lists Doze as Windows Settings and rejects its window binding with
  `window ... no longer belongs to ...; current owner is ...`, reporting the same
  owner on both sides. Fresh selection, helper reset, and application restart did
  not resolve it. The installed build also reproduces it with a fresh window ID.
  Chrome and the NSIS completion window bind successfully. Interactive mouse/keyboard flows,
  tray navigation, notifications, and actual countdown button clicks remain unverified.
  This tool binding failure persisted through three resumed goal turns and remains
  reproducible after the latest goal continuation. No pending QA session is left active.
- Actual Windows sign-in/reboot,
  multi-monitor/DPI, device switching/Bluetooth, long-running resource usage, and
  macOS hardware behavior were not verified.
- Reading all Windows power requests requires administrator privileges and was
  unavailable. A successful live session response confirms the native wake API
  accepted the request, not an independent administrator-level power audit.

The user selected Sleep as the only real power action. Hibernate, Lock, Display
Off and Shutdown remain mocked, as requested; they are not outstanding physical
power tests for this run.

## Repeating live agent QA

With Doze running, MCP enabled, and an existing Codex profile:

```powershell
node apps/desktop/scripts/qa-live-agent.mjs return_to_normal
```

To exercise the installed binary rather than the checkout's debug executable:

```powershell
$env:DOZE_QA_BINARY = "$env:LOCALAPPDATA\Doze\doze.exe"
node apps/desktop/scripts/qa-live-agent.mjs return_to_normal
```

The runner reads the existing local credential without displaying it or modifying
permissions. Approve Allow Once in Doze, then enter `get`, `heartbeat`, and
`finish`. `cancel` and `fail` test those explicit terminal outcomes. `quit` closes
the transport; it never reports success or cancels an active session implicitly.
An uncompleted session becomes connection-lost when its lease expires.

For a specifically authorized hardware Sleep test, pass `sleep` instead.
`finish` may start the real five-minute Sleep countdown. A person must be available
to wake and unlock the machine for recovery checks.

While the debug app is running, use a separate Cargo target directory for lint and
tests to avoid the native companion's Windows file lock. MCP integration supports
`DOZE_MCP_TEST_RELEASE=1` for the same reason.

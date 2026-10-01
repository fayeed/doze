# macOS QA — 1 October 2026

First run of Doze on a Mac: macOS 27.0.1 on Apple Silicon, Xcode with the macOS 27 SDK,
Rust 1.97. Interactive checks drove the real app through the Accessibility API (Swift AX
helper and System Events). The menu bar menu itself cannot be opened through
Accessibility, so its actions were exercised through the Settings control center, which
sends the same engine operations. Screen capture needs Screen Recording permission, so
visual review used `native:render` and headless Chrome for the website.

## Found and fixed

| Problem | Fix |
| --- | --- |
| Two skill installer tests failed: `/var` is a link to `/private/var`, and every ancestor was rejected | Only components below the trusted root (home or Doze's data folder) are checked; new test refuses a link below the root |
| Tray status permanently read "Needs attention: Suspend notifications unavailable" | IOKit sleep/wake observer (`IORegisterForSystemPower`) |
| Launch at login, audio sessions, After Playback, Lock, Display Off and Shut down were unavailable | LaunchAgent, Core Audio playback state, login framework lock, `pmset displaysleepnow`, loginwindow shut down request |
| `pnpm build:desktop` failed: stripped proc-macro dylibs rejected by dyld ("mis-aligned LINKEDIT string pool") | `[profile.release.build-override] strip = false`; shipped binary still stripped |
| Opening the running app from Finder or Spotlight did nothing visible | Reopen event shows Settings |
| New users saw only a menu bar icon | First launch opens Settings and saves defaults |
| Labels "Shutdown"/"Displayoff", raw lease seconds and agent statuses | Shared action labels, "5 minutes", "Waiting for your approval" |
| VoiceOver: number fields read their label twice; identical "Start for 15m" buttons; a decorative moon announced as "Snooze" | Single labels, "Keep awake for 15m" / "Sleep in 15m", icons hidden |
| Custom Power Timer did not show which action would run | Action picker, presets, stepper, end-time preview |
| Live QA runner was Windows-only | macOS paths and `DOZE_QA_CLIENT` |

## Verified live

| Area | Evidence |
| --- | --- |
| Checks | 87 Rust tests, 6 Node lease tests, MCP SDK integration against the real stdio binary, native `--verify-ui`, workspace lint/test/build, and Windows-target clippy (`x86_64-pc-windows-msvc`, warnings denied) all passed |
| Keep Awake | 15m start, Extend (15 → 30 minutes), Stop and Indefinitely; `pmset -g assertions` showed `PreventUserIdleDisplaySleep "Doze keep awake"` owned by Doze, switching to `PreventUserIdleSystemSleep` when display sleep was allowed, and no assertion after Stop |
| Power Timer and final warning | 1-minute Turn display off timer held the Mac awake; the floating warning appeared after about 61 seconds with a live 0:14; Snooze hid it and moved the deadline to 15:04; Cancel cleared it and released the assertion. A second run's Stay Awake cleared the timer and continued as Keep Awake indefinitely |
| Preview | 60-second preview of the selected action, "Preview only", no Stay Awake, Cancel dismissed it with no engine change |
| Audio | Core Audio probe: idle reported not running; `afplay` set device and process output running; muted output (this Mac's state) correctly counted as silence. Both audio toggles ran without monitoring errors |
| Launch at login | Toggle wrote a valid LaunchAgent (`plutil -lint`) for the current executable with `--startup`; toggling off removed it |
| Suspend observer | Registered at startup; the permanent error disappeared |
| Menu bar time | "15m" appeared beside the icon during a 15-minute session; the preference removed and restored it live |
| Agents | Claude Code profile setup showed the macOS command, endpoint and key; a real stdio session waited for approval with no assertion, Allow Once made it active and held the assertion, heartbeat renewed the lease, finish released it and the runner exited cleanly |
| Bundle | `Doze.app` and DMG built; universal companion in Resources, `LSUIElement` and Apple Events usage in Info.plist; the bundled app used its bundled companion and reopened to Settings |

## Follow-up hardware run (same day, with the user present)

Screen Recording was granted to VS Code, so the real screen, menu and windows were captured.
The installed `Doze.app` bundle was used throughout.

| Area | Evidence |
| --- | --- |
| Audible playback | Output unmuted at 6%: the Core Audio probe reported meaningful audio during speech and none after. Keep awake while audio plays held `Doze keep awake` from about 2s into speech until the 10s silence grace ended. Mute and volume were restored |
| After Playback → Display Off | Countdown appeared 18s after playback began (speech plus 10s silence, user idle); `pmset -g log` recorded "Display is turned off" at the end of the 15s warning; After Playback re-armed |
| Lock | Lock timer locked the screen on schedule (`CGSSessionScreenIsLocked`), released Doze's assertion; the user unlocked 5s later |
| Sleep | Three Doze-triggered sleeps logged `PMRD: sleep reason Software Sleep` immediately after Doze released its assertion, entered dark wake, and were woken 2–4s later by trackpad activity tickles inside powerd's ~5s dark-wake linger. Apple's `pmset sleepnow` produced the identical kernel sequence and completed deep sleep when left alone. Overview's last event read "System suspended or resumed · sessions cleared", proving the IOKit observer |
| Launch at login | `launchctl bootstrap` of the LaunchAgent started Doze with `--startup` in the menu bar without Settings; Background Task Management lists it as "Doze" after the attribution fix |
| Notifications | macOS asked to allow "Doze" notifications; once allowed, a real countdown showed "Doze · Power countdown — Turn display off in 0m 15s. Open Doze in the menu bar…" |
| Menu bar | Real menu: status rows, submenu arrows, ⌘, and ⌘Q; "0:13" beside the icon during a countdown |
| Windows | Settings sidebar and toolbar glass, the countdown's glass button group, and the timer window captured from the screen in dark mode |
| Shut down | A Shut down timer's request reached loginwindow ("Received a kAEShutDown", logout with no extra UI) at 19:33:09; the Mac shut down and booted at 19:33:59. No Automation consent prompt was needed for this event |
| Real login launch | Launch at login was on during that restart: after boot, launchd started Doze from the LaunchAgent with `--startup`, in the menu bar without Settings |

Found and fixed in this run: "Help  About" lost its ampersand (macOS strips single `&`
mnemonics too); the Settings sidebar opened at 144pt and truncated page names; the custom
timer window's transparent frame left its buttons floating; and the login item was named
"doze" in background-activity notices. Overview now also shows the last engine event.

Menu item icons do not appear in Doze's menu, and a plain AppKit status menu with a named
system image and an SF Symbol shows none either, so this is macOS 27 behaviour.


The setting had been re-enabled at 19:26:36 by a click on its switch. Replaying every
scripted step from that time, and rapid toggle bursts (50 ms to 1 s apart), never changed it
unexpectedly; the switch, saved setting and LaunchAgent always agreed.

## Still requiring verification on a Mac

- Bluetooth/AirPlay output switching, keyboard navigation of the menu bar menu, and the
  light appearance on screen.

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

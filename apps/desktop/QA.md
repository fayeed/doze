# macOS QA — 10 October 2026

MacBook Pro (MacBookPro18,1) on macOS 27, on AC power, no external display. The debug build
ran from its LaunchAgent; the panel, Settings, timer and warning windows were driven with real
mouse events and Accessibility, and checked against screen captures, `pmset -g assertions`,
`pmset -g log` and the settings file.

## Found and fixed

| Problem | Fix |
| --- | --- |
| The panel's Countdown, Quick Settings and Help & About rows hid the panel and opened a native menu | They open pages inside the panel, with a back button and Esc; the panel resizes under the icon |
| Dark square corners around the panel: the window shadow took the glass backdrop as a full rectangle | Panel content is clipped to its rounded shape |
| "Set up…" in the panel used the system link blue | Accent colour |
| The panel's "Keep the display on" switch never changed | Switches save the opposite of the stored value |
| A closed MacBook slept while an agent held Doze (every lid close logged "Clamshell Sleep") | While Doze holds the Mac it sets IOPMrootDomain's clamshell override, without root |
| After a fresh start, Settings opened from the panel listed no Claude Code, Codex, OpenCode or Gemini CLI rows | Opening Settings asks the engine for a full snapshot |
| Connecting an agent always meant Doze editing its config | "Use a prompt…" gives a per-agent prompt the agent runs itself (macOS and Windows) |
| The panel's "No agents running" card was cramped | 8 pt above and below |

## Verified live

| Area | Evidence |
| --- | --- |
| Panel pages | Quick Settings, Countdown and Help & About open in place and Back returns; right-click still opens the full menu; Menu guide opens Settings on that page; Preview from the Countdown page closed the panel and showed "Preview only" |
| Keep Awake | 15m: "15m left · until 8:46 AM", "15m" beside the icon, `PreventUserIdleDisplaySleep "Doze keep awake"`; Indefinitely: "Until you stop it"; Stop released it |
| Power Timer | Custom 1-minute "Turn display off" timer from the timer window; the warning appeared 61 s later; the panel's countdown card snoozed it 15 minutes, then Cancel released the assertion |
| Settings | Panel switches and pop-ups wrote `logging`, `countdownSeconds` and `allowDisplaySleep`; turning the display switch off moved the held assertion to `PreventUserIdleSystemSleep` at once |
| MCP approval | A client without permission waited in `awaiting_authorization` with the attention glyph and an approval card; Allow made it active, heartbeats every 20.1 s were accepted, and finish released it with no action |
| MCP completion | A pre-approved client finished with `display_off`: "All agents finished" ran its 5-minute warning (agents never get less), then the display turned off at 08:51:50 as Doze released its assertion |
| Hooks | `doze hook claude-code UserPromptSubmit` asked for approval ("doze · QA: run the test suite"); after Allow it showed Working; `Stop` released the Mac |
| CLI | `doze run` held the Mac while the command ran, released it after, and returned the command's status (0, and 3 for a failing command) |
| Display off | With an agent working, `pmset displaysleepnow` kept the display off for 2 minutes (08:40:38 to 08:42:40, no input): heartbeats continued every 20.1 s and nothing slept. Repeated for 90 s with the display allowed to sleep |
| Lid closed | With an agent working, the lid was closed from 08:55:45 to 09:00:36: no Clamshell Sleep, sleep or wake in `pmset -g log`, heartbeats every 20.1 s throughout. Finishing the session cleared the override and its record file |
| Use a prompt | The sheet showed Claude Code's prompt for the real settings path; Copy Prompt put it on the clipboard. An agent given the prompt for a scratch settings file backed it up, kept every other entry and added exactly Connect's entries. Cancelling Connect's preview left `~/.claude/settings.json` unchanged |
| Checks | 116 Rust tests, 6 Node lease tests, MCP SDK integration, clippy with warnings denied, the universal companion build and `--verify-ui` (now covering the panel pages and the prompt sheet) |

## Still requiring verification

- Other apps held idle-sleep assertions during the display-off test (a Transporter upload and
  coreaudiod for simulator audio), so it shows Doze's assertion held and the agent kept
  running, not that Doze alone prevented idle sleep. Assertions never stop lid-close sleep, so
  the lid test is unaffected.
- Lid closed on battery, and with an external display attached (where Doze leaves the override
  to powerd), were not tested.
- The Windows "Use a prompt…" button, dialog and verification were written on a Mac without the
  .NET SDK; run `scripts/test-native.ps1` on Windows. Windows-target clippy also needs Windows
  (`llvm-rc`).
- Rebuilding `target/debug/doze` in place while the LaunchAgent copy runs gets the next launch
  killed (`OS_REASON_CODESIGNING`); copy the binary to a new file first. Development only.
- Not changed: MCP sessions that never call `update_session` show "Done · 0m" worked, and the
  Connect preview lists added lines before the line they replace.

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
| Light appearance | Doze's Light theme over a dark system: Settings captured from the screen with correct contrast and full sidebar labels |
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

- Bluetooth/AirPlay output switching (only the built-in speakers were approved for testing)
  and keyboard navigation of the menu bar menu (synthetic keystrokes were avoided).

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

# Windows Settings parity QA — 1 October 2026

Windows 11 Pro on x64, one 2560×1440 display at 100% scaling, .NET SDK 8.0.425, Rust 1.96.
The WinUI companion was brought to parity with the redesigned macOS companion and restyled
after Windows 11 Settings. The real app was the current release build of `doze.exe` with the
new companion, driven through UI Automation (the tree a screen reader uses). Screenshots of
the real windows used `PrintWindow`, which captures only Doze's own window. The keep-awake
request was checked with `CallNtPowerInformation(SystemExecutionState)`, which needs no
administrator rights; `powercfg /requests` needs elevation and this session was not
elevated. Another app held the display throughout, so the baseline state was `0x2`
(display required); Doze adds `0x1` (system required).

## Found and fixed

| Problem | Fix |
| --- | --- |
| `doze run` from PowerShell returned in 3 ms with no exit status or output: `doze.exe` is a GUI program | `doze-cli.exe`, a console front end installed beside it that waits and returns the job's status |
| Overview showed status text only; the custom Power Timer had no action picker | Control center and a timer window with action, presets and an end preview |
| Durations were number fields; leases read 300; statuses read `connection lost` | Menus of friendly values; "5 minutes", "Connection lost · keeping awake", "Waiting for your approval" |
| Reset applied immediately | Confirmation dialog, Cancel by default, says agent connections and sessions are kept |
| Search matched page names only | Keyword search inside pages |
| Re-measuring cards in `SizeChanged` caused a WinUI layout cycle (`0xC000027B`) at render time | Card controls are re-placed only when the width changes, using their natural width; unhandled WinUI exceptions are now reported instead of a bare crash code |
| A `JsonValue` created from an `int` did not read as `long`, hiding Extend in verification | Numbers are parsed from their JSON text |

## Verified

| Area | Evidence |
| --- | --- |
| Checks | `native:build`, `native:test`, `test` (92 Rust tests, 6 Node lease tests), `mcp:test` (official MCP SDK against the real stdio binary, including command-line jobs and keep-alive), `lint` passed. `format:check` passes for every changed file; locally Prettier still flags four untouched files that git stores with LF but this `core.autocrlf=true` checkout writes with CRLF (`--end-of-line auto` passes) |
| Native verification | Nine pages in light/dark; no number fields, engine ids, raw seconds or repeated titles; Overview with idle, active and final-warning sessions, with distinct preset names ("Keep awake for 15m", "Sleep in 15m") and in-place clock updates; Agents keep-alive, lease labels, statuses and a Command line job; About icons and links; Reset confirmation; keyword search; the timer window's action, presets (pressed through their automation peer), request fields and preview |
| Visual review | Every page rendered in both themes at 1120×780, at 883×475 (a 1080p display at 200% scaling) and at the 680×480 minimum, plus scrolled ends, Overview idle/busy states, dialogs, four timer forms and the countdown states, 142 images. Each render also fails on truncated text, controls narrower than their content, or cards extending past the page. Renders were reviewed by eye. Higher DPI changes rasterization, not layout in effective pixels, so the 200% case is covered by its effective size |
| Keep Awake | Overview "Keep awake for 15m": status "Keeping awake · 15m left", state `0x2` → `0x3`; Extend 15 minutes → "30m left · ends 22:09"; Stop → "Normal sleep allowed", state back to `0x2` |
| Power Timer and final warning | Action set to Turn display off, More → Custom duration…, duration 1 minute, preview "Turn display off at 21:41, after the final warning", Start Timer → "Turn display off scheduled · 42s left", state `0x3`. The floating warning appeared on time ("Turn display off in 4:46", Stay Awake present) and Overview showed its Final warning section. Cancel action released the request (`0x2`) |
| Stay Awake | A second 1-minute timer reached its warning ("Turn display off in 5:00 · From the Power Timer"); Stay Awake on Overview turned it into "Keeping awake · indefinitely / Until you stop it" (`0x3`); Stop released it |
| Audio switches | "Keep awake while audio plays" and "Sleep after playback stops" switched on and off through the engine; After Playback's description showed "Waiting for playback to start" while on |
| Agents | "Keep sessions alive while connected" saved `false` then `true` in settings.json. A `doze-cli run` job appeared as "Command line · Running ping -n 20 127.0.0.1 · Working · When finished: Return to normal" (`0x3`); Cancel session released it (`0x2`) and the job exited 1 |
| Advanced and Reset | The Command line section showed the doze-cli.exe path, alias and example; Copy example put the exact command on the clipboard. Reset… opened "Reset all preferences?" with Cancel as the default; Cancel left settings.json byte-identical |
| About | Doze's and Clypy's icons from the bundled Assets; Source Code opened github.com/fayeed/doze in the browser; Open data folder opened Explorer at `%APPDATA%\app.getdoze.desktop`; Visit Clypy launched without error |
| Themes on screen | Real Mica windows captured in dark (system) and Light; the theme was restored to "Use system setting" |
| Command line, PowerShell | `doze-cli run --then nothing -- cmd /c "ping … & echo job-done"` waited 6 s, printed Doze's lines and the job's output, exit 0; a job exiting 3 returned 3 with "Doze will not sleep"; `watch --pid` waited 8 s for a running ping, exit 0; a missing pid exited 1 with a message; no arguments printed the usage (exit 0) and an unknown command printed it with exit 2 |
| Command line, cmd.exe | In a real console window, `doze-cli run --then nothing -- cmd /c "timeout /t 5"` counted down 5…0, exit 0; a job exiting 7 returned 7 (checked with delayed `!errorlevel!`) |
| Ctrl-C | A real `GenerateConsoleCtrlEvent` in the job's console: ping printed "Control-C", Doze reported "exited with status -1073741510; Doze will not sleep" and released, and doze-cli returned that status |

## Follow-up run (same evening)

| Area | Evidence |
| --- | --- |
| After Playback one-shot | Temporarily Turn display off, 10 s silence, 30 s idle, 1-minute warning. Armed: "Waiting for playback to start"; 8 s of `Alarm01.wav` → "Playing · waiting for it to stop"; 6 s after it stopped → "Waiting for silence and inactivity"; at 33 s idle the warning appeared ("Turn display off in 0:57 · After playback stopped"). Cancel turned the switch off, with the last event "Power action cancelled · After Playback turned off". The preferences were then restored byte for byte |
| Keyboard | Real keystrokes (sent only while a Doze window was in front). Tab reaches every control on every page in reading order; Space on "Keep awake for 15m" started a session and Enter on "Stop keeping awake" ended it; Menu guide scrolls to its end with End. In the timer window, opened by keyboard (Enter on More, Enter on Custom duration…), focus starts in Duration, Tab cycles presets, Start Timer, Cancel and Action, and Esc closes it |
| Installer | `pnpm build:desktop` produced `Doze_0.1.0_x64-setup.exe` (59.37 MiB). A silent per-user install (`/S`, exit 0) put `doze.exe`, `doze-cli.exe` and `windows-ui\` (with `Assets\Doze.png` and `Assets\Clypy.png`) in `%LOCALAPPDATA%\Doze`. The installed `doze-cli.exe` passed the PowerShell and cmd.exe checks above (watch, `timeout /t 5`, status 7); Advanced showed the installed path, and the copied `Set-Alias doze …` ran a job |

Found and fixed in this run: expander cards shared the accessible name of the switch in their
header ("Allow agents to connect" twice) and several session buttons shared names; after a
keyboard action rebuilt Overview, focus jumped to the search box; and Menu guide could not be
scrolled with the keyboard. Expanders are now "More options for …", session buttons name their
client, rebuilds restore focus to the same control or its section, text-only pages are a tab
stop, and verification fails on duplicate names.

## Still requiring verification

- Narrator's speech. The names, roles and states it reads were checked through UI
  Automation, but nobody listened to Narrator itself.
- Physical 150%/200% displays. Changing the scale would have rescaled the user's whole
  screen; layouts were verified at the equivalent effective window sizes instead.
- An interactive cmd.exe prompt does not wait for GUI programs; `cmd /c` scripts do. Plain
  `doze.exe run` therefore still returns early at an interactive prompt, which is why
  Advanced and the README point to `doze-cli.exe`.

## Sleep under Away Mode — 2 October 2026

A Sleep from Doze at 01:15 the night before left the desktop unlocked: the System log
showed `doze.exe` calling `SetSuspendState`, then "The system is entering Away Mode", and
Windows Update and time-sync events all night with no sleep or resume. `powercfg /requests`
(administrator) listed Logitech G HUB's `lghub_agent.exe` under AWAYMODE ("G HUB is shutting
down"), and the Balanced plan's "Allow Away Mode Policy" is on when plugged in.

Requests made with `PowerSetRequest`, like G HUB's, and `SetThreadExecutionState` Away Mode
requests were both reproduced and neither appears in `CallNtPowerInformation`'s system
execution state, so Doze now watches `GUID_SYSTEM_AWAYMODE` during a Sleep request instead,
and locks the workstation before Sleep or Hibernate.

Live check with the reinstalled build: a test process held an Away Mode request the way G HUB
does, the warning was set to 15 seconds and a 1-minute Sleep timer started. The lock screen
appeared at 08:43:56, Kernel-Power 187 (caller `doze.exe`) and 59 "entering Away Mode" followed
at 08:43:57, and after the user signed in at 08:44:10 Overview's last event read "Windows
stayed on in Away Mode instead of sleeping because another app requested it (powercfg
/requests names it) · desktop locked". The test request was released and the 5-minute warning
restored.

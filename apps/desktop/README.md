# Doze desktop

A local tray (Windows) and menu bar (macOS 13+) utility built with Tauri 2 and Rust. No account, cloud services, telemetry, or simulated input.

## Run and verify

From the repository root:

```sh
pnpm dev:desktop
pnpm --filter @doze/desktop format
pnpm --filter @doze/desktop format:check
pnpm --filter @doze/desktop lint
pnpm --filter @doze/desktop test
pnpm build:desktop
```

Requires Rust 1.88+, plus Microsoft C++ desktop build tools and a .NET 8 SDK on Windows, or Xcode with a macOS 26+ SDK on macOS. The native build script also recognizes a workspace-local SDK at `.tools/dotnet/`. The app starts in the tray by default. Left-click or right-click opens the native system menu. Settings, About, custom durations and specific dates/times use WinUI 3 with Mica on Windows. Closing a dialog keeps sessions running. Quit releases the native power request.

Build output is in `src-tauri/target/release/`: `doze.exe`, the `doze-cli.exe` console front end, its `windows-ui/` companion folder, and `bundle/nsis/`. Windows uses NSIS: MSI validation rejects language IDs in the bundled Microsoft runtime DLLs. Installers are unsigned development artifacts.

`pnpm --filter @doze/desktop native:build` publishes the self-contained WinUI companion and builds `doze-cli.exe` (`native/cli`). `pnpm --filter @doze/desktop native:test` constructs all nine pages in light/dark modes, the Overview control center with idle, active and final-warning sessions, and the four custom timer forms, and verifies queued immediate edits, failure rollback, reset confirmation, keyword search and wording (no engine ids or raw seconds) without showing a window or writing preferences. `powershell -File scripts/test-native.ps1 -RenderDirectory <folder>` also renders every page, dialog and window in both themes at the default size, at a 1080p display's 200% scaling and at the minimum window size, failing on truncated text or clipped controls. Tauri dev/build hooks publish it automatically. Installers include the .NET and Windows App SDK runtimes. An unpackaged distribution must keep the `windows-ui` folder and `doze-cli.exe` beside `doze.exe`; the EXE alone is no longer a complete distribution.

## Behavior

The clickable status rows open native help. “Normal sleep allowed” means Doze is not preventing Windows from sleeping; ordinary Windows power settings apply. Inactive controls explain their reason in the menu, and “Help & About → Menu Guide” explains every feature, checked state, unavailable action, and current timing setting.

The tray uses native icon menu items with antialiased line glyphs, short live status rows, and separators between session, timer, and preference groups. Keep Awake contains Extend/Stop; Power Timer contains action, duration, and Stop; Countdown contains Cancel/Snooze/Preview; Help & About contains the Menu Guide and About. Checkmarks retain their native toggle meaning. Settings and Quit show platform keyboard shortcuts. On macOS, applicable items carry AppKit's built-in icons, though macOS 27 does not draw item images in menu bar menus (a plain AppKit status menu behaves the same), and the tray glyph uses template rendering for the menu bar's appearance. While a countdown, power timer or timed Keep Awake session runs, the macOS menu bar shows the time left beside the icon (General → "Show time remaining in the menu bar").

- Keep Awake: 15m, 30m, 1h, 2h, custom duration, a local date/time, or indefinitely. Timed sessions can be extended or stopped.
- Control center: Settings → Overview starts, extends and stops Keep Awake, schedules and stops Power Timers with a chosen action, toggles the audio rules, and handles a running final warning (Snooze 15 minutes, Stay Awake, Cancel) with live times, on both platforms. It refreshes every second while something has a deadline and every five seconds otherwise; on Windows, ticks that only change times update text in place so focus and scrolling survive. The custom timer window has an action picker, presets (15m–8h) and an end-time preview.
- First launch opens Settings and saves defaults; later launches and launches at login stay in the tray or menu bar. On macOS, opening Doze again from Finder, Spotlight or Launchpad shows Settings, as a second launch does on Windows.
- Keep awake while audio plays: retains the power request through the configured silence grace period.
- Sleep Timer: supported native actions with presets or custom duration/date/time. The selected duration is followed by the common countdown. The timer holds the computer awake until it finishes.
- After Playback: three meaningful observations in distinct seconds arm the rule. Silence alone cannot arm it. After the silence grace and required idle duration, a countdown starts. Clicking Pause/Stop or using the computer during the silence wait restarts the inactivity wait without disarming the rule. Resumed audio restarts the silence wait; input during a visible countdown cancels it. Observation failures or device changes cancel the playback action. Cancellation requires fresh playback to arm again. The rule is one-shot: when its action runs, or the user chooses Cancel or Stay Awake on its warning, After Playback turns itself off, so it must be turned on again (for example each night) and cannot trip during the day. Automatic cancellations from input or resumed audio leave it armed.
- Countdown: Windows uses a native WinUI/Mica window matching Settings, with a separate action heading, 72-point tabular timer, and native buttons. macOS uses its own SwiftUI/AppKit window with a 64-point timer. Real countdowns and previews stay above ordinary windows. Cancel (including Escape or closing the warning) removes the action. Snooze adds 15 minutes, hides the window for that interval, then restores it; a new countdown appears immediately. The window hides when the countdown is cleared or completed. Native notifications remain optional while the warning window works. If the window cannot be shown, Doze always sends the notification and the countdown continues with Cancel/Snooze in the tray; if no notification can be shown either, the countdown is cancelled. Notification action buttons are not implemented.
- Preview countdown: opens a 60-second, always-on-top demonstration for the selected timer action. It stays visible at zero until dismissed. Cancel and Snooze dismiss the preview without submitting engine operations. A visible real countdown takes priority over a preview.
- An explicit Sleep Timer takes precedence over After Playback. Manual Keep Awake blocks playback-triggered actions. An explicit timer can end a manual keep-awake session.
- Defaults: 60s silence, 300s idle, 300s countdown, Sleep. Unsupported actions are disabled; saved defaults are normalized to supported actions.
- Appearance: General has System, Light, and Dark choices, saved and applied immediately. System is the default for new and existing installations and detects the OS appearance when native windows open. Windows listens for system color changes; macOS inherits the current system appearance when no override is selected. Settings, About, Menu Guide, and countdown windows share the selected theme.
- Settings: a native sidebar grouped like the system's settings (Overview; General, Session defaults, After playback, Notifications; Agents, Advanced; Menu guide and About), search that matches settings inside pages, and cards. On Windows the cards follow Windows 11 Settings: an icon, a title with its description underneath and the control on the right (moving under the text in narrow windows), grouped under section headers, with expander cards for sub-options. Durations are chosen from menus of friendly values that keep any saved custom value. Valid changes apply immediately, with no Save/Discard bar on either platform. Rapid edits are queued without losing newer values; rejected changes restore confirmed preferences and show an error. Reset defaults lives in Advanced, asks for confirmation and keeps agent connections, permissions and sessions. The Windows companion embeds Doze's icon, assigns it to the window, and enables taskbar/Alt+Tab visibility.
- Quick Settings: saved checkboxes for display sleep, notifications, launch at sign-in, starting in the tray, and diagnostic logging, plus default awake/timer durations. Changes persist immediately. Duration defaults apply to new sessions; unrelated preferences preserve existing timers and playback state.
- About Doze: Doze's own icon, version, tagline, author, Source Code and data-folder links, an "Also from the developer" card for Clypy with its own icon, privacy and acknowledgements. Both icons are bundled files; nothing is fetched from the network.
- Allow display sleep: keeps the system awake while allowing Windows to turn off the screen. It updates an active power request immediately. Defaults are 30 minutes for both awake and timer shortcuts.

Only settings persist, in Tauri's per-user configuration directory (`settings.json`). Writes use a flushed temporary file and atomic rename. Startup and local error logging are opt-in. Logs record changed errors and are bounded to roughly 256 KiB. Reset defaults disables launch at startup. Each launch reconciles the login item with the saved setting: it re-registers the current executable (so a moved app keeps working) or removes a stale item, writing only when something changed. Older settings files receive defaults for newly added preferences. A settings file that cannot be read or validated is copied to `settings.invalid.json` before Doze falls back to defaults, so later saves never destroy it.

Sessions and power actions never restore after restart. Windows suspend/resume events clear manual timers and audio sessions. Agent sessions instead enter connection-lost state and retain their wake request for up to 30 minutes; excessive scheduler delays do the same. Wall-clock changes alone (time sync, manual changes) never affect sessions unless native suspend notifications are unavailable, in which case they are treated as a possible suspend. Specific times become monotonic durations when scheduled.

## Architecture

| Source | Responsibility |
| --- | --- |
| `src-tauri/src/core/` | Platform-independent sessions, rules, countdowns, and transition tests |
| `src-tauri/src/mcp/` | Stdio protocol, authenticated local bridge, tool validation, client authorization and lease models |
| `src-tauri/src/state/` | Single-owner runtime, validated operations, settings persistence |
| `src-tauri/src/platform/` | PowerManager, AudioMonitor, IdleMonitor, NotificationManager adapters |
| `src-tauri/src/tray.rs` | Native tray menus, checked states, status and countdown controls |
| `src-tauri/src/quick_settings.rs` | Native shortcuts to saved preferences and default durations |
| `native/windows/` | Native WinUI preferences, control center, About, timers and countdown |
| `native/cli/` | `doze-cli.exe`, the Windows console front end for `doze run` and `doze watch` |
| `native/macos/Doze.swift` | Native SwiftUI/AppKit preferences, About, timers, and countdown |
| `src-tauri/src/platform/native_ui.rs` | Private platform UI pipes; engine validation and persistence |

One channel-driven worker owns sessions, COM interfaces, and power requests. Native windows submit validated operations through the worker channel. The WinUI companion uses private inherited standard-I/O pipes; it has no power-action implementation or network endpoint. The hidden app without audio monitoring wakes at most hourly or at a deadline; lifecycle events and commands wake it immediately. Audio meters and countdowns use one-second observations. Tray labels update when the worker wakes or the tray is clicked. Endpoint changes use OS callbacks. No React, HTML, CSS, JavaScript frontend, webview window, or browser interval is used. Node is only needed for development tooling.

## Native Windows APIs

| Feature | API |
| --- | --- |
| Settings / About | WinUI 3 companion; Rust owns validation and persistence |
| Countdown | WinUI 3, matching Mica and theme resources; native always-on-top presenter |
| Custom timers | WinUI 3 Mica, action picker, number/date/time pickers, presets, live end-time preview, inline validation, and theme resources |
| Keep Awake | `SetThreadExecutionState` with a continuous system requirement and optional display requirement; released on the same worker thread |
| Sleep / Hibernate | `GetPwrCapabilities`, `LockWorkStation` first, then `SetSuspendState`; a `GUID_SYSTEM_AWAYMODE` notification detects Away Mode |
| Shutdown | `InitiateSystemShutdownExW`; temporary `SeShutdownPrivilege`, restored after execution; no forced app closure |
| Lock | `LockWorkStation` |
| Display off | `WM_SYSCOMMAND / SC_MONITORPOWER` with bounded `SendMessageTimeoutW` |
| Idle | `GetLastInputInfo`, wrap-safe `GetTickCount` |
| Audio | Core Audio `IMMDeviceEnumerator`, `IAudioMeterInformation`, `IAudioEndpointVolume`, `IMMNotificationClient` |
| Notifications | Tauri's native notification plugin |
| Startup | Quoted executable path in current-user `Software\Microsoft\Windows\CurrentVersion\Run` |
| Suspend/resume | `PowerRegisterSuspendResumeNotification` with an owned callback context |

Before Sleep or Hibernate, Doze locks the desktop, so you return to the lock screen even when Windows doesn't require sign-in after sleep. When any app holds an Away Mode request and the power plan's "Allow Away Mode Policy" is on (the default for Balanced on many desktops), Windows turns a sleep request into Away Mode: the screen goes dark and sound mutes but the computer keeps running. Doze watches for that and records it as the last event ("Windows stayed on in Away Mode instead of sleeping…"). `powercfg /requests` in an administrator terminal lists the app under AWAYMODE; Logitech G HUB is a known example. Turning the policy off (`powercfg /setacvalueindex SCHEME_CURRENT SUB_SLEEP AWAYMODE 0`, then `powercfg /setactive SCHEME_CURRENT`) makes sleep real again. Requests made with `PowerSetRequest` do not appear in the system execution state, so Doze cannot predict Away Mode before asking to sleep.

Audio meters observe all active render endpoints, including non-default devices. Peak output above -60 dBFS after endpoint mute/volume checks counts as meaningful. Silent sessions do not count. Muted playback cannot arm a fresh rule; muting previously detected playback is treated as silence. No media content is recorded or inspected.

[Microsoft documents](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudiometerinformation) that software peak meters report zero in exclusive mode. Exclusive playback without hardware meters, very quiet content, and short bursts between samples are limitations. Protected/exclusive media and Bluetooth transitions need hardware validation before treating After Playback as dependable for every media source.

## Native appearance

Windows Settings, About, Menu Guide, countdown and custom timers share WinUI 3 Mica Alt and native theme resources. Settings follows Windows 11 Settings: navigation with coloured Segoe Fluent icons that collapses to icons in narrow windows, a fixed page title, section headers over groups of cards spaced 4 px apart, On/Off switches, menus and expander cards. Navigation keeps the window backdrop intact and resets page scrolling. Custom session popups use native pickers, inline errors, and a dialog-style footer with Start Timer or Keep Awake and Cancel. The system tray menu remains OS-rendered. Hidden native windows keep the private UI pipe available, with no UI polling timer while hidden. Restart Doze after updating the native companion to load the new binary.

macOS uses native SwiftUI/AppKit windows: sidebar preferences, About, custom durations/end times, and a floating countdown with Cancel/Snooze. Native navigation and controls adopt the current OS design when built against its SDK. Command surfaces use `glassEffect` on macOS 26+ and native material on older releases; system accessibility preferences govern transparency and contrast. Settings and About use the same window shell. The menu bar continues to use native macOS menus, template status icons, grouped actions, checkmarks, and Command shortcuts. Windows uses its native tray menu and WinUI controls. References: [Windows materials](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type), [Apple's Liquid Glass guidance](https://developer.apple.com/documentation/technologyoverviews/adopting-liquid-glass).

Build macOS on a Mac with Xcode and a macOS 26 or newer SDK selected through `xcode-select`. `pnpm --filter @doze/desktop native:build` compiles a universal Swift companion for Apple Silicon and Intel; `native:test` constructs the native pages in light/dark without saving preferences or performing power actions; `native:render [folder]` draws every page and window offscreen to PNG files for visual review (screen capture would need Screen Recording permission). `pnpm build:desktop` produces `Doze.app` and a DMG with the companion in `Contents/Resources/macos-ui` and an `Info.plist` that hides the Dock icon (`LSUIElement`) and declares Apple Events use for Shut down. Bundles are ad-hoc signed development artifacts. Use the newest available Xcode SDK for the latest system appearance.

The app icon and tray glyphs come from the brand artwork in `apps/web/public/brand`. Windows uses the freestanding setting sun (`doze-icon-windows.svg`) for `icon.ico` and the PNGs; macOS uses the sun inside Apple's padded 824 px rounded-rectangle grid (`doze-icon-macos.svg`) for `icon.icns`. Sizes of 32 px and below use the simplified `-16-32` drawings. After changing the artwork, run `pnpm --filter @doze/desktop icons` (needs Chrome or `CHROME_PATH`). The tray draws `doze-glyph-normal`, `-awake` and `-countdown` in code (`tray::image`): a ring while normal sleep is allowed, a filled sun while awake and the striped setting sun during a countdown; the shapes differ so macOS template rendering keeps them distinct.

## Native macOS APIs

| Feature | API |
| --- | --- |
| Settings / About / timers / countdown | SwiftUI and AppKit companion over private pipes; Rust owns validation and persistence |
| Keep Awake | `IOPMAssertionCreateWithName`: `PreventUserIdleDisplaySleep`, or `PreventUserIdleSystemSleep` when the display may sleep |
| Sleep | `IOPMSleepSystem` |
| Shut down | Standard `aevt/shut` request to loginwindow; apps with unsaved documents can stop it. Verified on macOS 27 without an Automation prompt |
| Lock | Login framework `SACLockScreenImmediate`, resolved at runtime; Lock is offered only when present |
| Display off | `pmset displaysleepnow` |
| Hibernate | Not offered: macOS does not let apps request hibernation |
| Idle | `CGEventSourceSecondsSinceLastEventType` |
| Audio | Core Audio object state: an output device running (`kAudioDevicePropertyDeviceIsRunningSomewhere`), not muted and above zero volume; on macOS 14+ a process must be running output (`kAudioProcessPropertyIsRunningOutput`) |
| Startup | Per-user LaunchAgent `~/Library/LaunchAgents/app.getdoze.desktop.plist` starting the current executable with `--startup` |
| Suspend/resume | `IORegisterForSystemPower` on a dedicated run loop thread |

macOS playback detection captures and inspects no audio and asks for no recording permission. Unlike Windows peak meters, a silent stream that keeps an output device running counts as playback, and very quiet content counts too. Muted or zero-volume output counts as silence. Device or default-output changes cancel a playback action, as on Windows. Read-only probe: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml audio::tests::native_read_only_probe -- --ignored --nocapture`.

The Rust engine remains the only owner of countdown time and power execution. Preview controls submit no power operation; losing the native companion cancels any pending real countdown.

## Hardware verification

Tests cover silence-only behavior, chimes, grace restarts, idle gating, resumed audio/input at the execution deadline, failures, cancellation, snoozing, precedence, wake-session expiry, and transient-session clearing. Tests do not execute disruptive power actions.

Read-only Windows probe:

```sh
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml native_read_only_probe -- --ignored --nocapture
```

Before release, verify native menu and dialog keyboard navigation/layout across monitor/DPI/taskbar configurations; Stop/Quit/crash power-request release; Modern Standby and power plans; every countdown action including privilege refusal and unsaved documents; browser/player audio, buffering, mute, silent sessions, quiet content, exclusive audio, headphones and Bluetooth; installed notifications and Focus Assist; startup after installation/reboot; invalid settings, clock changes, lid close, suspend/resume, and pending-shutdown restart; long-running CPU/RAM; and on macOS, real Sleep/Shut down/Lock/Display Off execution, audible playback detection with unmuted output, notifications from an installed bundle, and an actual login with launch at login enabled.

## Agents and MCP

Open **Doze → Agents → Agent settings and connections**, turn on **Enable MCP**, and choose **Set up** for Codex, Claude Code, or a generic MCP client. A setup dialog shows the configuration, a Copy button, and instructions for that client. Existing profiles have **Configure** to reopen setup and **Permissions** to manage automatic grants using switches. Creating a profile does not install or verify the client connection. MCP is off by default. Doze generates a separate random credential for each profile. Leave the desktop app running. A remotely hosted model works when its MCP client process runs on this computer; a remote execution machine cannot control this Doze instance through stdio.

The first request opens Agents settings for **Allow Once** or **Deny**. Approval grants Keep Awake plus the exact requested completion action. Allow Once is scoped to that session. Persistent permissions can only be changed explicitly under Trusted agents and permissions in Settings. Future requests for other actions require approval. An awaiting-authorization session does **not** hold a wake assertion: the agent must poll `doze.get_session` and wait for `active` before claiming the computer is protected. Pending approvals expire after ten minutes.

### Codex

Copy the Codex configuration into your user `~/.codex/config.toml` (or the configuration location managed by your installation), then restart/reload the client. It has this shape; use the actual paths and key displayed by Doze:

```toml
[mcp_servers.doze]
command = "C:\\Program Files\\Doze\\doze.exe"
args = ["--mcp", "--endpoint", "C:\\Users\\you\\AppData\\Roaming\\app.getdoze.desktop\\mcp-endpoint.json"]
[mcp_servers.doze.env]
DOZE_MCP_KEY = "<credential copied from Doze>"
```

This follows the documented [Codex stdio configuration](https://developers.openai.com/codex/mcp). Doze does not modify the client's files automatically or assume where its executable is installed. On macOS, the generated command points to the app bundle executable.

### Claude Code

Copy the Claude Code JSON object displayed by Doze and add it using the documented [JSON configuration command](https://code.claude.com/docs/en/mcp#add-an-mcp-server-from-json):

```sh
claude mcp add-json --scope user doze '<JSON copied from Doze>'
claude mcp get doze
```

The object includes `type: "stdio"`, `command`, `args`, and `env.DOZE_MCP_KEY`. Use shell-appropriate quoting for the JSON, especially with Windows paths. Alternatively add it as the `doze` value in your client's `mcpServers` configuration. Keep credentials in user configuration rather than committing them to a project.

### Generic clients

Copy the generic JSON configuration into the client's MCP settings, or supply the generated `command`, `args` and `env` to its stdio launcher. MCP uses newline-delimited JSON-RPC over inherited stdin/stdout. It supports the `2025-11-25` lifecycle, with negotiation for `2024-11-05`, `2025-03-26` and `2025-06-18`. Clients using a newer protocol must support fallback negotiation. The transport follows the [MCP stdio specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports). There is no remotely reachable HTTP MCP endpoint.

### Install the companion skill

Every Doze build embeds the portable [`Doze skill`](../../skills/doze/SKILL.md) and its Codex metadata, so installation works offline. In an agent's **Set up / Configure** dialog, choose **Install Doze skill** for Codex or Claude Code. The dialog shows the destination and detects an identical installed copy. If the bundled files differ, **Review update** asks for confirmation before replacing them. Updates retain extra files and preserve the complete original in a `doze-skill-backups/<unique-id>` folder beside the client's `skills` directory. Local instruction edits are not merged; the completion message gives the backup path. Backups are outside the discovery directory to prevent duplicate skills. Linked or oversized folders require manual installation.

**Open skill folder** exports the bundled folder into Doze's data directory and opens it in Explorer/Finder for copying into other clients. The skill contains no credentials or runtime hooks and grants no power permissions. Configure MCP separately using the connection instructions above. Reload the client if the skill does not appear. The repository also ships `skills/doze` for manual installation:

Copy the `doze` folder into the appropriate personal skill directory:

| Client | Destination | Explicit invocation |
| --- | --- | --- |
| Codex | `~/.agents/skills/doze/` | `$doze` |
| Claude Code | `~/.claude/skills/doze/` | `/doze` |

These locations follow the [official OpenAI skill documentation](https://learn.chatgpt.com/docs/build-skills) and [Claude Code skill documentation](https://code.claude.com/docs/en/skills). For project-only installation, use `.agents/skills/doze/` or `.claude/skills/doze/` inside the target project. Check an existing folder before replacing it to preserve local edits.

From a Doze checkout, this PowerShell example installs a new Codex copy and refuses to overwrite an existing one:

```powershell
$dozeSkillSource = Join-Path (Get-Location) 'skills/doze'
$dozeSkillParent = Join-Path $HOME '.agents/skills'
$dozeSkillDestination = Join-Path $dozeSkillParent 'doze'
if (!(Test-Path -LiteralPath (Join-Path $dozeSkillSource 'SKILL.md'))) {
    throw 'Run this from the Doze repository root.'
}
if (Test-Path -LiteralPath $dozeSkillDestination) {
    throw 'Doze skill already exists; review it before updating.'
}
New-Item -ItemType Directory -Path $dozeSkillParent -Force | Out-Null
Copy-Item -LiteralPath $dozeSkillSource -Destination $dozeSkillDestination -Recurse
```

For Claude Code, change `.agents/skills` to `.claude/skills`. Reload or restart the client if the skill is absent. Example: “Use Doze to keep this computer awake while you finish the task, then put it to sleep.” Installing or loading the skill does not grant power permissions: Doze still requires Allow Once or an explicit Settings grant. The skill cannot guarantee heartbeat or Stop handling when the model is no longer running; see [Heartbeats without model turns](#heartbeats-without-model-turns).

### Agent lifecycle

Example user request: “I'm going to bed. Keep my computer awake while you finish this task, then put it to sleep.”

1. Call `doze.start_session` with `reason` and `completion_action: "sleep"`. An optional `optional_timeout` sets a maximum session duration in seconds (30–604800). If omitted, the completion behavior comes from Agents settings, initially Return to Normal. Supported values are `return_to_normal`, `sleep`, `hibernate`, `lock`, `display_off`, and `shutdown`; platform support and local permission are always checked.
2. If authorization is pending, ask the user to approve in Doze, then poll `doze.get_session`. The response includes `session_id`, configured client identity, status, requested/authorized action and lease timestamps. All timestamps are monotonic seconds since the current Doze process started, not Unix timestamps.
3. Call `doze.heartbeat(session_id)` at least twice per lease interval. The default lease is five minutes; configurable range is 30–3600 seconds. Renewal never extends the optional total timeout.
4. Call `doze.finish_session(session_id)` only after the work and validation are complete. It marks explicit completion in the engine. It never performs a power action directly.
5. Call `doze.fail_session(session_id)` only when the runtime definitively reports task failure. It releases that authorized lease without executing its action. Successful peers can still trigger their unanimously authorized completion action; a batch consisting only of failed tasks never triggers one. Silence or disconnection is not definitive failure.
6. Call `doze.cancel_session(session_id)` to release a session without its action. `doze.get_session` and `doze.list_sessions` expose only sessions owned by the configured client. Reconnecting with the same profile credential can resume an existing lease. Use separate profiles for separate trust boundaries.

The existing native engine owns the wake assertion, warning countdown and OS action. Only authorized active and connection-lost sessions block Doze power countdowns, including existing timers. Manual awake/audio sessions also defer agent completion. An enabled After Playback rule takes precedence even while waiting for playback to begin. Pending or denied requests do not enter authorized batches and cannot alter timers or countdowns. Paused playback warnings restart after authorized agent leases end, provided the silence/idle checks still pass. When the entire overlapping batch finishes explicitly with the same authorized action, the core starts a countdown of at least five minutes. **Cancel**, **Stay Awake**, and **Snooze** remain available. Return to Normal, cancellation, or differing completion actions veto the automatic action for that batch. No ranking silently escalates from Sleep to Shutdown.

Lease expiration, transport EOF, total timeout, suspend/resume, or a clock discontinuity is uncertainty rather than proof of completion. A lost lease conservatively keeps the computer awake indefinitely. The tray and Agents settings show connection lost and last activity, with **Cancel session**, **Wait 30 minutes**, or **End and apply completion action**. A valid heartbeat can recover a lost lease unless its total timeout elapsed. A resumed explicit finish can resolve it. No CPU, process, network, editor, or task-completion heuristics are used.

Changing a profile's permissions cancels its active sessions and any pending agent completion. **Revoke connection** invalidates its key and cancels its sessions. Disabling MCP cancels agent sessions and agent countdowns. General settings edits and Reset defaults preserve agent credentials and permissions; use the Agents controls to revoke or disable them. Session state never persists across an app restart; normal OS sleep behavior resumes until a new authorized session is created.

### Local security boundary

Each tool call requires a profile key. Claimed MCP `clientInfo.name` is not used as an authorization identity. The private bridge listens only on an ephemeral IPv4 loopback port and requires a separate per-launch random token read from the per-user endpoint file. Profile credentials live in local settings; Unix files are created with mode 0600, and Windows files inherit the user's configuration-directory ACL. Keep that directory and client configuration private. Like other local stdio integrations, this does not isolate hostile processes running as the same OS user that can read those files. The MCP interface exposes no arbitrary commands or immediate power-action tools. Input sizes, concurrent bridge requests, client profiles, active sessions and overlapping batch history are bounded.

### Verification

```sh
pnpm --filter @doze/desktop test
pnpm --filter @doze/desktop mcp:test
pnpm --filter @doze/desktop native:test
pnpm --filter @doze/desktop format:check
pnpm lint
pnpm test
pnpm build
```

`mcp:test` uses the official TypeScript MCP client SDK against the actual Doze stdio binary, connected to a test-only engine host with MockPower. It checks handshake/tool discovery, session creation and mock assertion, UI snapshot data, heartbeat renewal, finish/countdown, user cancellation, overlapping clients, conflicting actions, ownership and invalid credentials, unauthorized shutdown, disconnect/expiry and reconnect. Rust tests cover authorization decisions, leases, timers, wake arbitration, timeout, failure state and cancellation. Native UI tests construct the Agents approval, lost-connection, permissions and connection controls in light/dark themes. The mock host is excluded from normal builds and installers by a required `mcp-test-support` Cargo feature. Tests never execute native sleep/shutdown. Real native power transitions require separate platform hardware validation.

On Windows, if a running debug app locks `doze.exe`, set `$env:DOZE_MCP_TEST_RELEASE="1"` in PowerShell before running `mcp:test`. The test then builds and uses the release binaries without closing the desktop app.

### Keep-alive while connected

The `doze --mcp` process that the client launches renews the sessions its agent created or used while the client keeps that stdio connection open, so one long step without model turns (a build, a conversion) does not lose the lease. It renews three times per lease and never finishes a session. Renewal stops when the client exits or crashes, after which the normal lease expiry and 30-minute connection-lost grace apply, and Doze refuses it 24 hours after the agent's own last heartbeat so a forgotten finish cannot hold the computer indefinitely. Turn it off with **Agents › Keep sessions alive while the agent app is connected**. An open pipe is evidence that the client is alive, not that work succeeded, so completion still requires an explicit `finish_session`.

## Command-line jobs

`doze run` keeps the computer awake while a command runs and can sleep afterwards. `doze watch` follows a process that is already running:

```sh
doze run --then sleep -- ffmpeg -i in.mov out.mp4
doze watch --pid 1234 --then sleep
```

`--then` takes `nothing` (default), `sleep`, `display-off`, `lock`, `shutdown` or `hibernate`, limited to the actions the computer supports. On macOS, use the desktop binary as `doze`: `/Applications/Doze.app/Contents/MacOS/doze` (Settings › Advanced copies a shell alias). On Windows, use `doze-cli.exe` from the install folder (Settings › Advanced shows the path and copies a PowerShell alias such as `Set-Alias doze '…\doze-cli.exe'`). Doze must be running; MCP does not need to be enabled.

A job is a session from the built-in **Command line** client, listed with agent sessions in Agents and the tray. It needs no approval because the user started it, and its calls travel only over the private bridge, whose per-launch token is in the user's `0600` endpoint file. The command's exit status decides the outcome: `0` finishes the job, so the action follows the usual final warning (at least five minutes, with Cancel, Snooze and Stay Awake); any other status, a signal or Ctrl-C reports failure and releases the computer without acting. `doze run` exits with the command's status. `doze watch` cannot read another process's exit status, so its exit counts as success. If `doze` itself is killed, the lease expires and the 30-minute connection-lost grace applies. Jobs overlap with agent sessions in the same batch, so a job with `--then nothing` vetoes an agent's automatic action and vice versa. On Windows, `doze.exe` is a GUI program, so shells do not wait for it or keep its exit status; `doze-cli.exe` is a console program that starts `doze.exe` with the same arguments and terminal, waits, and returns the job's status. It ignores Ctrl-C only after starting the job, so the job receives it and Doze releases the computer. Anything other than `run`, `watch` or `help` shows the usage instead of starting the app.

### Heartbeats without model turns

The lease remains the failure detector; removing renewal would leave Doze blind between start and finish. `scripts/runtime-lease.mjs` provides the opt-in `watchDozeRun` adapter for runtimes using an MCP client. It polls a supplied **fresh authoritative** job-status callback, renews while that job is explicitly running, and finishes only when the callback reports `succeeded`. These MCP/status calls do not invoke the language model. A definitive `failed` job state calls `doze.fail_session`, releasing that lease without executing its requested action. Disconnected status, user-input waits, unknown state, a hung status callback, or abort stop renewal without reporting success. Doze then enters connection-lost state when the remaining lease expires. A cached running flag, an open MCP pipe, or a background helper that lives indefinitely is insufficient evidence.

```js
import { watchDozeRun } from "./scripts/runtime-lease.mjs";

const result = await watchDozeRun(mcpClient,
  { reason: "Finish the requested refactor", completion_action: "sleep" },
  { readStatus: async ({ signal }) => runtime.readFreshJobStatus(jobId, { signal }) });
// readFreshJobStatus is your runtime adapter, not a Doze or MCP standard method.
// It returns running/succeeded only for authoritative whole-job states.
```

No Codex/Claude hooks are installed automatically. A provider integration must distinguish a successful whole job from an individual turn ending, child tasks still running, cancellation, or waiting for approval. A runtime event stream plus its documented connection/status checks can implement this callback. Generic clients without those signals continue using explicit heartbeats. Longer leases reduce calls but increase failure-detection latency; every uncertainty favors keeping awake rather than executing a completion action.

For example, with three authorized agents, one can finish successfully at 15 minutes, a second can explicitly report terminal model failure, and the third can finish successfully at an hour. The first two no longer hold wake leases; the third keeps the machine awake. Its successful finish begins the common countdown when the successful sessions agree on their authorized action. If the second agent disappears instead of reporting failure, its lease becomes connection-lost and continues to block sleep for up to 30 minutes, until resolved or a heartbeat recovers it. After that the session is cancelled without any completion action, which also vetoes the batch's automatic action. This distinction is intentional.

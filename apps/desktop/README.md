# Doze desktop

A local tray utility built with Tauri 2 and Rust. Windows is the first supported target. No account, cloud services, telemetry, or simulated input.

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

Requires Rust 1.88+, Microsoft C++ desktop build tools, and a .NET 8 SDK on Windows. The native build script also recognizes a workspace-local SDK at `.tools/dotnet/`. The app starts in the tray by default. Left-click or right-click opens the native system menu. Settings and About use WinUI 3; custom durations and specific dates/times use native Windows dialogs. Closing a dialog keeps sessions running. Quit releases the native power request.

Build output is in `src-tauri/target/release/`: `doze.exe`, its `windows-ui/` companion folder, and `bundle/nsis/`. Windows uses NSIS: MSI validation rejects language IDs in the bundled Microsoft runtime DLLs. Installers are unsigned development artifacts.

`pnpm --filter @doze/desktop native:build` publishes the self-contained WinUI companion. `pnpm --filter @doze/desktop native:test` constructs all nine pages in light/dark modes and verifies queued immediate edits, failure rollback and reset without showing a window or writing preferences. Tauri dev/build hooks publish it automatically. Installers include the .NET and Windows App SDK runtimes. An unpackaged distribution must keep the `windows-ui` folder beside `doze.exe`; the EXE alone is no longer a complete distribution.

## Behavior

The clickable status rows open native help. “Normal sleep allowed” means Doze is not preventing Windows from sleeping; ordinary Windows power settings apply. Inactive controls explain their reason in the menu, and “Help & About → Menu Guide” explains every feature, checked state, unavailable action, and current timing setting.

The tray uses native icon menu items with antialiased line glyphs, short live status rows, and separators between session, timer, and preference groups. Keep Awake contains Extend/Stop; Power Timer contains action, duration, and Stop; Countdown contains Cancel/Snooze/Preview; Help & About contains the Menu Guide and About. Checkmarks retain their native toggle meaning. Settings and Quit show platform keyboard shortcuts. On macOS, applicable items use AppKit's built-in icons and the tray glyph uses template rendering for the menu bar's appearance. macOS rendering still requires verification on a Mac.

- Keep Awake: 15m, 30m, 1h, 2h, custom duration, a local date/time, or indefinitely. Timed sessions can be extended or stopped.
- Keep awake while audio plays: retains the power request through the configured silence grace period.
- Sleep Timer: supported native actions with presets or custom duration/date/time. The selected duration is followed by the common countdown. The timer holds the computer awake until it finishes.
- After Playback: three meaningful observations in distinct seconds arm the rule. Silence alone cannot arm it. After the silence grace and required idle duration, a countdown starts. Resumed audio, user activity during grace/countdown, observation failures, or device changes cancel the playback action. Cancellation requires fresh playback to arm again.
- Countdown: Windows uses a native WinUI/Mica window matching Settings, with a separate action heading, 72-point tabular timer, and native buttons. macOS uses its own SwiftUI/AppKit window with a 64-point timer. Real countdowns and previews stay above ordinary windows. Cancel (including Escape or closing the warning) removes the action. Snooze adds 15 minutes, hides the window for that interval, then restores it; a new countdown appears immediately. The window hides when the countdown is cleared or completed. Native notifications remain optional. Notification action buttons are not implemented.
- Preview countdown: opens a 60-second, always-on-top demonstration for the selected timer action. It stays visible at zero until dismissed. Cancel and Snooze dismiss the preview without submitting engine operations. A visible real countdown takes priority over a preview.
- An explicit Sleep Timer takes precedence over After Playback. Manual Keep Awake blocks playback-triggered actions. An explicit timer can end a manual keep-awake session.
- Defaults: 60s silence, 300s idle, 300s countdown, Sleep. Unsupported actions are disabled; saved defaults are normalized to supported actions.
- Appearance: General has System, Light, and Dark choices, saved and applied immediately. System is the default for new and existing installations and detects the OS appearance when native windows open. Windows listens for system color changes; macOS inherits the current system appearance when no override is selected. Settings, About, Menu Guide, and countdown windows share the selected theme.
- Settings: a native WinUI sidebar, search, and cards for Overview, General, Session Defaults, After Playback, Notifications, Agents, Advanced, Menu Guide, and About. ToggleSwitch, NumberBox, and ComboBox controls include explanations and supported-action selectors. Valid changes apply immediately, with no Save/Discard bar on either platform. Rapid edits are queued without losing newer values; rejected changes restore confirmed preferences and show an error. Reset defaults lives in Advanced and applies immediately. Overview refreshes live status every five seconds while visible. The Windows companion embeds Doze's icon, assigns it to the window, and enables taskbar/Alt+Tab visibility.
- Quick Settings: saved checkboxes for display sleep, notifications, launch at sign-in, starting in the tray, and diagnostic logging, plus default awake/timer durations. Changes persist immediately. Duration defaults apply to new sessions; unrelated preferences preserve existing timers and playback state.
- About Doze: native version/platform information, product purpose, privacy, local data location, and acknowledgements in the same WinUI window.
- Allow display sleep: keeps the system awake while allowing Windows to turn off the screen. It updates an active power request immediately. Defaults are 30 minutes for both awake and timer shortcuts.

Only settings persist, in Tauri's per-user configuration directory (`settings.json`). Writes use a flushed temporary file and atomic rename. Startup and local error logging are opt-in. Logs record changed errors and are bounded to roughly 256 KiB. Reset defaults disables launch at startup. Older settings files receive defaults for newly added preferences.

Sessions and power actions never restore after restart. Windows suspend/resume events clear manual timers and audio sessions. Agent sessions instead enter connection-lost state and retain their wake request; wall-clock discontinuities or excessive scheduler delays do the same. Specific times become monotonic durations when scheduled.

## Architecture

| Source | Responsibility |
| --- | --- |
| `src-tauri/src/core/` | Platform-independent sessions, rules, countdowns, and transition tests |
| `src-tauri/src/mcp/` | Stdio protocol, authenticated local bridge, tool validation, client authorization and lease models |
| `src-tauri/src/state/` | Single-owner runtime, validated operations, settings persistence |
| `src-tauri/src/platform/` | PowerManager, AudioMonitor, IdleMonitor, NotificationManager adapters |
| `src-tauri/src/tray.rs` | Native tray menus, checked states, status and countdown controls |
| `src-tauri/src/quick_settings.rs` | Native shortcuts to saved preferences and default durations |
| `native/windows/` | Native WinUI preferences and About |
| `native/macos/Doze.swift` | Native SwiftUI/AppKit preferences, About, timers, and countdown |
| `src-tauri/src/platform/native_ui.rs` | Private platform UI pipes; engine validation and persistence |

One channel-driven worker owns sessions, COM interfaces, and power requests. Native windows submit validated operations through the worker channel. The WinUI companion uses private inherited standard-I/O pipes; it has no power-action implementation or network endpoint. The hidden app without audio monitoring wakes at most hourly or at a deadline; lifecycle events and commands wake it immediately. Audio meters and countdowns use one-second observations. Tray labels update when the worker wakes or the tray is clicked. Endpoint changes use OS callbacks. No React, HTML, CSS, JavaScript frontend, webview window, or browser interval is used. Node is only needed for development tooling.

## Native Windows APIs

| Feature | API |
| --- | --- |
| Settings / About | WinUI 3 companion; Rust owns validation and persistence |
| Countdown | WinUI 3, matching Mica and theme resources; native always-on-top presenter |
| Custom timers | Win32 native dialogs and date/time pickers; alpha-correct themed text over system glass |
| Keep Awake | `SetThreadExecutionState` with a continuous system requirement and optional display requirement; released on the same worker thread |
| Sleep / Hibernate | `GetPwrCapabilities`, `SetSuspendState` |
| Shutdown | `InitiateSystemShutdownExW`; temporary `SeShutdownPrivilege`, restored after execution; no forced app closure |
| Lock | `LockWorkStation` |
| Display off | `WM_SYSCOMMAND / SC_MONITORPOWER` with bounded `SendMessageTimeoutW` |
| Idle | `GetLastInputInfo`, wrap-safe `GetTickCount` |
| Audio | Core Audio `IMMDeviceEnumerator`, `IAudioMeterInformation`, `IAudioEndpointVolume`, `IMMNotificationClient` |
| Notifications | Tauri's native notification plugin |
| Startup | Quoted executable path in current-user `Software\Microsoft\Windows\CurrentVersion\Run` |
| Suspend/resume | `PowerRegisterSuspendResumeNotification` with an owned callback context |

Audio meters observe all active render endpoints, including non-default devices. Peak output above -60 dBFS after endpoint mute/volume checks counts as meaningful. Silent sessions do not count. Muted playback cannot arm a fresh rule; muting previously detected playback is treated as silence. No media content is recorded or inspected.

[Microsoft documents](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudiometerinformation) that software peak meters report zero in exclusive mode. Exclusive playback without hardware meters, very quiet content, and short bursts between samples are limitations. Protected/exclusive media and Bluetooth transitions need hardware validation before treating After Playback as dependable for every media source.

## Native appearance

Windows Settings, About, Menu Guide and countdown share WinUI 3 Mica Alt and native theme resources. Settings has sidebar navigation, search, cards, toggles, number fields and action selectors. The guide uses the native Library icon. Navigation keeps the window backdrop intact and resets page scrolling. Custom timer windows extend DWM glass across their client area and paint labels with composited alpha; editable fields remain opaque for readability. The system tray menu remains OS-rendered. Hidden Settings/countdown windows keep the private UI pipe available, with no UI polling timer while hidden. Restart Doze after updating the native companion to load the new binary.

macOS uses native SwiftUI/AppKit windows: sidebar preferences, About, custom durations/end times, and a floating countdown with Cancel/Snooze. Native navigation and controls adopt the current OS design when built against its SDK. Command surfaces use `glassEffect` on macOS 26+ and native material on older releases; system accessibility preferences govern transparency and contrast. Settings and About use the same window shell. The menu bar continues to use native macOS menus, template status icons, grouped actions, checkmarks, and Command shortcuts. Windows uses its native tray menu and WinUI controls. References: [Windows materials](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type), [Apple's Liquid Glass guidance](https://developer.apple.com/documentation/technologyoverviews/adopting-liquid-glass).

Build macOS on a Mac with Xcode and a macOS 26 or newer SDK selected through `xcode-select`. `pnpm --filter @doze/desktop native:build` compiles a universal Swift companion for Apple Silicon and Intel; `native:test` constructs the native pages in light/dark without saving preferences or performing power actions. `pnpm build:desktop` includes the companion in the app's Resources/macos-ui folder. Native macOS sources and packaging have not been compiled or visually verified from this Windows workspace. Use the newest available Xcode SDK for the latest system appearance.

Separate adapters include IOKit keep-awake assertions and Sleep, Core Graphics idle detection, and native notifications through Tauri. These have not been compiled or tested on a Mac. After Playback, launch at login, and native suspend/resume observation remain unimplemented on macOS; controls are disabled and the limitation is reported. Other macOS power actions are disabled. The Rust engine remains the only owner of countdown time and power execution. Preview controls submit no power operation; losing the native companion cancels any pending real countdown.

## Hardware verification

Tests cover silence-only behavior, chimes, grace restarts, idle gating, resumed audio/input at the execution deadline, failures, cancellation, snoozing, precedence, wake-session expiry, and transient-session clearing. Tests do not execute disruptive power actions.

Read-only Windows probe:

```sh
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml native_read_only_probe -- --ignored --nocapture
```

Before release, verify native menu and dialog keyboard navigation/layout across monitor/DPI/taskbar configurations; Stop/Quit/crash power-request release; Modern Standby and power plans; every countdown action including privilege refusal and unsaved documents; browser/player audio, buffering, mute, silent sessions, quiet content, exclusive audio, headphones and Bluetooth; installed notifications and Focus Assist; startup after installation/reboot; invalid settings, clock changes, lid close, suspend/resume, and pending-shutdown restart; long-running CPU/RAM; and macOS compilation/native hardware behavior.

## Agents and MCP

Open **Doze → Agents → Agent settings and connections**, turn on **Enable MCP**, and choose **Connect** for Codex, Claude Code, or a generic MCP client. MCP is off by default. Doze generates a separate random credential for each profile and shows copyable configuration containing the installed executable and local endpoint paths. Leave the desktop app running. A remotely hosted model works when its MCP client process runs on this computer; a remote execution machine cannot control this Doze instance through stdio.

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

The repository ships a portable skill at [`skills/doze`](../../skills/doze/SKILL.md), including Codex UI metadata and a dependency on the `doze` MCP connection. Distribute this whole folder alongside a Doze release. It contains no credentials or runtime hooks. Configure MCP using the connection instructions above separately.

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

`mcp:test` uses the official TypeScript MCP client SDK against the actual Doze stdio binary, connected to a test-only engine host with MockPower. It checks handshake/tool discovery, session creation and mock assertion, UI snapshot data, heartbeat renewal, finish/countdown, user cancellation, overlapping clients, conflicting actions, ownership and invalid credentials, unauthorized shutdown, disconnect/expiry and reconnect. Rust tests cover authorization decisions, leases, timers, wake arbitration, timeout, failure state and cancellation. Native UI tests construct the Agents approval, lost-connection, permissions and connection controls in light/dark themes. The mock host is excluded from normal builds and installers by a required `mcp-test-support` Cargo feature. Tests never execute native sleep/shutdown. Real native power transitions and macOS compilation require separate platform hardware validation.

On Windows, if a running debug app locks `doze.exe`, set `$env:DOZE_MCP_TEST_RELEASE="1"` in PowerShell before running `mcp:test`. The test then builds and uses the release binaries without closing the desktop app.

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

For example, with three authorized agents, one can finish successfully at 15 minutes, a second can explicitly report terminal model failure, and the third can finish successfully at an hour. The first two no longer hold wake leases; the third keeps the machine awake. Its successful finish begins the common countdown when the successful sessions agree on their authorized action. If the second agent disappears instead of reporting failure, its lease becomes connection-lost and continues to block sleep until resolved. This distinction is intentional.

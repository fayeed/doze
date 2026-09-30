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

Requires Rust 1.88+, Microsoft C++ desktop build tools on Windows. The app starts in the tray by default. Left-click or right-click opens the native system menu. Settings, custom durations, and specific dates/times use standard Windows dialogs. Closing a dialog keeps sessions running. Quit releases the native power request.

Build output is in `src-tauri/target/release/`: `doze.exe`, `bundle/msi/`, and `bundle/nsis/`. Installers are unsigned development artifacts.

## Behavior

- Keep Awake: 15m, 30m, 1h, 2h, custom duration, a local date/time, or indefinitely. Timed sessions can be extended or stopped.
- Keep awake while audio plays: retains the power request through the configured silence grace period.
- Sleep Timer: supported native actions with presets or custom duration/date/time. The selected duration is followed by the common countdown. The timer holds the computer awake until it finishes.
- After Playback: three meaningful observations in distinct seconds arm the rule. Silence alone cannot arm it. After the silence grace and required idle duration, a countdown starts. Resumed audio, user activity during grace/countdown, observation failures, or device changes cancel the playback action. Cancellation requires fresh playback to arm again.
- Countdown: Cancel removes the action; Snooze adds 15 minutes. Native notifications announce the countdown and direct users to tray controls. Notification action buttons are not implemented.
- An explicit Sleep Timer takes precedence over After Playback. Manual Keep Awake blocks playback-triggered actions. An explicit timer can end a manual keep-awake session.
- Defaults: 60s silence, 300s idle, 300s countdown, Sleep. Unsupported actions are disabled; saved defaults are normalized to supported actions.

Only settings persist, in Tauri's per-user configuration directory (`settings.json`). Writes use a flushed temporary file and atomic rename. Startup and local error logging are opt-in. Logs record changed errors and are bounded to roughly 256 KiB. Reset restores defaults and disables launch at startup.

Sessions and power actions never restore after restart. Windows suspend/resume events clear transient sessions. Wall-clock discontinuities or excessive scheduler delays clear them too. Specific times become monotonic durations when scheduled.

## Architecture

| Source | Responsibility |
| --- | --- |
| `src-tauri/src/core/` | Platform-independent sessions, rules, countdowns, and transition tests |
| `src-tauri/src/state/` | Single-owner runtime, validated operations, settings persistence |
| `src-tauri/src/platform/` | PowerManager, AudioMonitor, IdleMonitor, NotificationManager adapters |
| `src-tauri/src/tray.rs` | Native tray menus, checked states, status and countdown controls |

One channel-driven worker owns sessions, COM interfaces, and power requests. Native dialogs submit validated operations through the worker channel. The hidden app without audio monitoring wakes at most hourly or at a deadline; lifecycle events and commands wake it immediately. Audio meters and countdowns use one-second observations. Tray labels update when the worker wakes or the tray is clicked. Endpoint changes use OS callbacks. No React, HTML, CSS, JavaScript frontend, webview window, or browser interval is used. Node is only needed for development tooling.

## Native Windows APIs

| Feature | API |
| --- | --- |
| Settings / custom timers | Win32 modal dialogs, standard controls and native date/time pickers |
| Keep Awake | `SetThreadExecutionState` with continuous system/display requirements; released on the same worker thread |
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

## macOS status

Separate adapters include IOKit keep-awake assertions and Sleep, Core Graphics idle detection, and native notifications through Tauri. These have not been compiled or tested on a Mac. Native settings/custom timer dialogs, After Playback, launch at login, and native suspend/resume observation remain unimplemented on macOS; controls are hidden or the limitation is reported. Other macOS power actions are hidden.

## Hardware verification

Tests cover silence-only behavior, chimes, grace restarts, idle gating, resumed audio/input at the execution deadline, failures, cancellation, snoozing, precedence, wake-session expiry, and transient-session clearing. Tests do not execute disruptive power actions.

Read-only Windows probe:

```sh
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml native_read_only_probe -- --ignored --nocapture
```

Before release, verify native menu and dialog keyboard navigation/layout across monitor/DPI/taskbar configurations; Stop/Quit/crash power-request release; Modern Standby and power plans; every countdown action including privilege refusal and unsaved documents; browser/player audio, buffering, mute, silent sessions, quiet content, exclusive audio, headphones and Bluetooth; installed notifications and Focus Assist; startup after installation/reboot; invalid settings, clock changes, lid close, suspend/resume, and pending-shutdown restart; long-running CPU/RAM; and macOS compilation/native hardware behavior.

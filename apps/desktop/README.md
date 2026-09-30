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

`pnpm --filter @doze/desktop native:build` publishes the self-contained WinUI companion. `pnpm --filter @doze/desktop native:test` constructs all seven pages in light/dark modes and verifies draft preservation/reset without showing a window or writing preferences. Tauri dev/build hooks publish it automatically. Installers include the .NET and Windows App SDK runtimes. An unpackaged distribution must keep the `windows-ui` folder beside `doze.exe`; the EXE alone is no longer a complete distribution.

## Behavior

The clickable status rows open native help. “Normal sleep allowed” means Doze is not preventing Windows from sleeping; ordinary Windows power settings apply. Inactive controls explain their reason in the menu, and “Help & About → Menu Guide” explains every feature, checked state, unavailable action, and current timing setting.

The tray uses native icon menu items with antialiased line glyphs, short live status rows, and separators between session, timer, and preference groups. Keep Awake contains Extend/Stop; Power Timer contains action, duration, and Stop; Countdown contains Cancel/Snooze/Preview; Help & About contains the Menu Guide and About. Checkmarks retain their native toggle meaning. Settings and Quit show platform keyboard shortcuts. On macOS, applicable items use AppKit's built-in icons and the tray glyph uses template rendering for the menu bar's appearance. macOS rendering still requires verification on a Mac.

- Keep Awake: 15m, 30m, 1h, 2h, custom duration, a local date/time, or indefinitely. Timed sessions can be extended or stopped.
- Keep awake while audio plays: retains the power request through the configured silence grace period.
- Sleep Timer: supported native actions with presets or custom duration/date/time. The selected duration is followed by the common countdown. The timer holds the computer awake until it finishes.
- After Playback: three meaningful observations in distinct seconds arm the rule. Silence alone cannot arm it. After the silence grace and required idle duration, a countdown starts. Resumed audio, user activity during grace/countdown, observation failures, or device changes cancel the playback action. Cancellation requires fresh playback to arm again.
- Countdown: a native platform warning window shows the action and remaining time. Cancel (including Escape or closing the window) removes the action; Snooze adds 15 minutes. The window closes when the countdown is cleared or completed. Native notifications remain optional. Notification action buttons are not implemented.
- Preview countdown: opens a 60-second demonstration of the warning window for the selected timer action. Its buttons only affect the preview and cannot trigger a power action. A real countdown takes priority over the preview.
- An explicit Sleep Timer takes precedence over After Playback. Manual Keep Awake blocks playback-triggered actions. An explicit timer can end a manual keep-awake session.
- Defaults: 60s silence, 300s idle, 300s countdown, Sleep. Unsupported actions are disabled; saved defaults are normalized to supported actions.
- Settings: a native WinUI sidebar, search, and cards for Overview, General, Session Defaults, After Playback, Notifications, Advanced, and About. ToggleSwitch, NumberBox, and ComboBox controls include explanations and supported-action selectors. Save applies edits; Discard reverts them. Reset fills in defaults and applies them only after Save. Overview refreshes live status every five seconds while visible.
- Quick Settings: saved checkboxes for display sleep, notifications, launch at sign-in, starting in the tray, and diagnostic logging, plus default awake/timer durations. Changes persist immediately. Duration defaults apply to new sessions; unrelated preferences preserve existing timers and playback state.
- About Doze: native version/platform information, product purpose, privacy, local data location, and acknowledgements in the same WinUI window.
- Allow display sleep: keeps the system awake while allowing Windows to turn off the screen. It updates an active power request immediately. Defaults are 30 minutes for both awake and timer shortcuts.

Only settings persist, in Tauri's per-user configuration directory (`settings.json`). Writes use a flushed temporary file and atomic rename. Startup and local error logging are opt-in. Logs record changed errors and are bounded to roughly 256 KiB. Saving reset defaults disables launch at startup. Older settings files receive defaults for newly added preferences.

Sessions and power actions never restore after restart. Windows suspend/resume events clear transient sessions. Wall-clock discontinuities or excessive scheduler delays clear them too. Specific times become monotonic durations when scheduled.

## Architecture

| Source | Responsibility |
| --- | --- |
| `src-tauri/src/core/` | Platform-independent sessions, rules, countdowns, and transition tests |
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
| Custom timers / countdown | Win32 native dialogs and date/time pickers; alpha-correct themed text over system glass |
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

Windows Settings and About share a WinUI 3 Mica Alt backdrop, native sidebar navigation, search, cards, toggles, number fields, and action selectors. Navigation keeps the window backdrop intact and resets page scrolling. Theme resources follow light/dark and accessibility settings. Custom timer and countdown windows extend DWM glass across their client area and paint labels with composited alpha, rather than covering the material with grey rectangles. Standard editable fields remain opaque for readability. The system tray menu remains OS-rendered.

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

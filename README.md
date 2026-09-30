# Doze

Doze is a lightweight desktop utility for keeping your computer awake when it should be awake, and letting it sleep when it should sleep. The Windows tray app lives in `apps/desktop`; the website remains a placeholder.

> Your computer knows when it's bedtime.

## Requirements

- Node.js 20.19 or newer
- pnpm 10 or newer
- Rust 1.88 or newer and Cargo
- Tauri desktop prerequisites for your operating system

For Windows, install the Microsoft C++ Build Tools with the **Desktop development with C++** workload. For macOS, install Xcode with a macOS 26 or newer SDK for the native SwiftUI/AppKit companion and current Liquid Glass appearance. The desktop currently targets Windows and macOS.

## Get started

```sh
pnpm install
pnpm dev
```

This starts the Next.js site and the Tauri desktop app. Open the website at the URL printed by Next.js. Desktop development starts a tray app and requires the platform prerequisites above. Click the Doze tray icon to open its native system menu. Settings and About use native WinUI 3; custom timers use native Windows dialogs.

To run one app at a time:

```sh
pnpm dev:web
pnpm dev:desktop
```

## Commands

| Command | Description |
| --- | --- |
| `pnpm dev` | Start website and desktop app |
| `pnpm dev:web` | Start the Next.js development server |
| `pnpm dev:desktop` | Start the Tauri desktop development app |
| `pnpm build` | Build all workspace apps |
| `pnpm build:web` | Build the website |
| `pnpm build:desktop` | Build the desktop app and native bundle |
| `pnpm lint` | Lint website TypeScript and desktop Rust |
| `pnpm test` | Run workspace tests, including the desktop Rust engine |

## Workspace

- `apps/desktop` — native tray menus, WinUI 3 settings, Tauri 2, and Rust
- `apps/web` — Next.js App Router placeholder
- `packages/brand` — shared product name, domain, and tagline
- `packages/config` — shared TypeScript compiler options

See [the desktop README](apps/desktop/README.md) for native API choices, verification commands, and remaining hardware/macOS validation. No MCP or marketing website is implemented.

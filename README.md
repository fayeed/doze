# Doze

Doze is a lightweight desktop utility for keeping your computer awake when it should be awake, and letting it sleep when it should sleep. The Windows tray and macOS menu bar app lives in `apps/desktop`; the landing page lives in `apps/web`.

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

This starts the Next.js site and the Tauri desktop app. Open the website at the URL printed by Next.js. Desktop development starts a tray (Windows) or menu bar (macOS) app and requires the platform prerequisites above. Click the Doze icon to open its native system menu. The first launch also opens Settings, whose Overview page doubles as a control center. Settings, About and custom timers use native WinUI 3 with Mica on Windows and SwiftUI/AppKit on macOS, with Liquid Glass on macOS 26+ and native material on older releases.

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
- `apps/web` — Next.js App Router landing page
- `packages/brand` — shared product name, domain, and tagline
- `packages/config` — shared TypeScript compiler options

See [the desktop README](apps/desktop/README.md) for native API choices, verification commands, and remaining hardware validation, and [QA.md](apps/desktop/QA.md) for the latest Windows and macOS QA runs.

## Coding agents

Doze supports local MCP clients through its existing session engine. Open **Agents** in Doze to enable MCP, connect Codex/Claude Code, and grant per-agent wake and completion permissions. Agents renew leases and explicitly report completion; lost connections keep the computer awake for up to 30 minutes, then release without a completion action. See the [desktop MCP connection guide](apps/desktop/README.md#agents-and-mcp) for configuration, lifecycle, security and safe integration tests.

The portable [Doze skill](skills/doze/SKILL.md) teaches agents authorization, heartbeat renewal, success, failure, cancellation, and overlapping sessions. See [skill installation](apps/desktop/README.md#install-the-companion-skill) for Codex and Claude Code. The skill and MCP connection are installed separately; runtime heartbeat and cancellation hooks require a provider integration.

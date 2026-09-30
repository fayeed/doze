# Doze

Doze is a lightweight desktop utility in the making. This repository contains the initial monorepo foundation for the desktop app and its website.

> Your computer knows when it's bedtime.

## Requirements

- Node.js 20.19 or newer
- pnpm 10 or newer
- Rust stable and Cargo
- Tauri desktop prerequisites for your operating system

For Windows, install the Microsoft C++ Build Tools with the **Desktop development with C++** workload and the WebView2 Runtime. For macOS, install Xcode Command Line Tools. Linux development additionally needs the Tauri system libraries; see the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/).

## Get started

```sh
pnpm install
pnpm dev
```

This starts the Next.js site and the Tauri desktop app. Open the website at the URL printed by Next.js. Desktop development launches a native window and requires the platform prerequisites above.

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
| `pnpm lint` | Lint and type-check all app TypeScript |
| `pnpm test` | Run workspace test tasks (none are defined yet) |

## Workspace

- `apps/desktop` — React, TypeScript, Vite, Tauri 2, and Rust
- `apps/web` — Next.js App Router placeholder
- `packages/brand` — shared product name, domain, and tagline
- `packages/config` — shared TypeScript compiler options

The desktop and web apps are placeholders. No sleep control, power management, tray, audio detection, idle detection, timers, backend, analytics, or authentication is implemented.

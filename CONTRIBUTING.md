# Contributing to Doze

Thanks for taking the time to improve Doze. Bug reports, clear feature proposals, documentation fixes and focused pull requests are welcome.

## Before you start

- Search [existing issues](https://github.com/fayeed/doze/issues) before opening a new one.
- For a substantial feature or a change to power behavior, open an issue first so we can agree on the behavior and safety expectations.
- Never post a security vulnerability, personal data, access token, signing key or private log in a public issue. Follow [SECURITY.md](SECURITY.md) for private reports.

## Set up the project

Read the platform prerequisites and setup steps in the [README](README.md#for-developers). Install dependencies and run the app for your platform:

```sh
pnpm install
pnpm dev
```

## Before opening a pull request

Run the checks relevant to your change:

```sh
pnpm lint
pnpm test
pnpm build:web
```

For Rust changes, also check formatting:

```sh
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo fmt --manifest-path apps/desktop/native/cli/Cargo.toml --check
```

Power behavior should stay explicit and cancellable. Include tests for changed engine behavior, and describe which operating system you used to verify native changes. UI changes should include a screenshot or short recording where practical. Keep each pull request focused and explain the user-visible reason for the change.

## Pull requests

Open a pull request against `main`. Describe the problem, the change, and how you checked it. Link related issues and call out any behavior that differs between Windows and macOS. Maintainers may request changes or close a proposal that does not fit Doze's scope or safety model.

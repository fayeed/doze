# Publish desktop downloads to Cloudflare

Run `pnpm release` from the repository root on the OS you want to release. Windows builds an x64 NSIS installer including the native Settings and CLI companions. macOS builds a signed, notarized universal DMG for Apple Silicon and Intel. Both use the existing Tauri build hooks.

## One-time configuration

1. Create an R2 bucket in your Cloudflare account and attach a public custom domain to it. The bucket must be publicly readable. An `r2.dev` URL works for testing; Cloudflare recommends a custom domain for production.
2. Set `bucket` and `publicBaseUrl` in the root `release.config.json`. These are public configuration, not credentials. Example:

   ```json
   {
     "bucket": "doze-releases",
     "publicBaseUrl": "https://downloads.example.com"
   }
   ```

3. With Node.js 22 or newer, run `pnpm install` and `pnpm exec wrangler login` on each release machine. Alternatively set `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_API_TOKEN` with Account / Workers R2 Storage / Edit access. The release script loads an optional root `.env.release`, which Git ignores. Never commit credentials.
4. Deploy the website once with the populated `release.config.json`. Every download button uses the same URL configuration, including the platform toggle. Future releases update the files at those URLs without needing a website rebuild.

`DOZE_R2_BUCKET` and `DOZE_DOWNLOAD_BASE_URL` can override the shared configuration. If using the URL override, set the same `DOZE_DOWNLOAD_BASE_URL` in the website's build environment (or `apps/web/.env.local` for local development). The website retains its existing GitHub fallback until a download origin is configured. The two Cloudflare URLs become available independently as each OS is first published; publish both before directing production visitors to both links.

## Release

```sh
pnpm release --dry-run
pnpm release
```

On Windows PowerShell where execution policy blocks `pnpm.ps1`, use `pnpm.cmd release`.

The command checks bucket access, builds, selects a fresh installer for the configured Tauri version, and uploads it to a versioned path containing its SHA-256 hash. It downloads and verifies that archive before updating the current OS's stable link, then downloads and verifies the stable link too:

- `/releases/latest/Doze-windows-x64-setup.exe`
- `/releases/latest/Doze-macos-universal.dmg`

The other OS is untouched. Archived builds remain available. An upload or verification failure exits nonzero; a failure before promotion leaves the previous stable download intact. A verification failure after promotion means the stable object was uploaded but could not be verified; check public access/cache rules and rerun. Do not run two releases for the same OS simultaneously.

Latest downloads have `Cache-Control: no-store`. Do not configure Cloudflare cache rules that override it; archives use immutable caching. Wrangler supports uploads up to 315 MB; the script stops before uploading a larger installer. Full public-download verification uses additional bandwidth. This command publishes desktop installers; it does not deploy the website or configure automatic in-app updates.

## Build prerequisites and signing

Windows needs the x64 MSVC Rust toolchain, Microsoft C++ Build Tools, and .NET 8 (the repository-local SDK is supported). macOS needs Xcode with the macOS 26+ SDK, a **Developer ID Application** certificate installed in the keychain, and `APPLE_SIGNING_IDENTITY` set to its identity name; the script installs the two Rust architecture targets. Provide either App Store Connect API credentials (`APPLE_API_ISSUER`, `APPLE_API_KEY`, and `APPLE_API_KEY_PATH`) or Apple ID credentials (`APPLE_ID`, app-specific `APPLE_PASSWORD`, and `APPLE_TEAM_ID`). You can put these environment variables in the ignored root `.env.release` file or configure them as CI secrets. The release command stops before building if the Apple credentials are missing. The native Swift companion is signed with the Developer ID identity, secure timestamp, and hardened runtime before Tauri signs the app and notarizes/staples the DMG. Uploading to R2 does not sign or notarize an app.

Unset `CARGO_TARGET_DIR` and `CARGO_BUILD_TARGET` for releases. The native companion hooks expect the repository's standard build paths. Run `pnpm release:test` to check artifact selection and publishing failure behavior without building or accessing Cloudflare.

References: [R2 public buckets](https://developers.cloudflare.com/r2/buckets/public-buckets/), [Wrangler R2 uploads](https://developers.cloudflare.com/r2/reference/wrangler-commands/), [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/), [Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/).

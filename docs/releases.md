# Publish desktop downloads to GitHub

Push a version tag such as `v0.1.0` to start the GitHub Actions release workflow. It immediately creates a draft GitHub Release with auto-generated notes, builds the Windows x64 installer and signed, notarized universal macOS DMG on separate hosted runners, uploads both assets, then publishes the release. If either build fails, the release stays as a draft. Website download buttons point to the assets on the latest published release.

## Prepare a release

1. Update `version` in `apps/desktop/src-tauri/tauri.conf.json`.
2. Commit and push that change.
3. Create and push the matching tag from any one device:

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

Use the version you set in `tauri.conf.json` in place of `0.1.0`. The workflow rejects a tag that does not match the app version. Do not separately run release builds on different computers for the same tag: one tag push starts both platform builds, and the release job waits for both before publishing. If a build fails, add or fix its missing configuration and rerun the failed workflow; it will reuse the draft release and replace any same-named assets.

## GitHub repository secrets

Windows builds need no signing secrets. To sign and notarize macOS releases, add these repository Actions secrets:

- `APPLE_CERTIFICATE_P12`: base64 encoded Developer ID Application `.p12` certificate.
- `APPLE_CERTIFICATE_PASSWORD`: password for the `.p12` file.
- `APPLE_SIGNING_IDENTITY`: certificate identity name.
- `APPLE_API_ISSUER`: App Store Connect issuer ID.
- `APPLE_API_KEY`: App Store Connect key ID.
- `APPLE_API_KEY_P8`: contents of the App Store Connect `.p8` key file.

The workflow imports the certificate into its temporary runner keychain and writes the API key into the runner's temporary directory. Nothing is committed to the repository. If any signing or notarization secret is absent or invalid, the macOS build fails and the release is not published.

## Package manager publishing

After a stable release publishes both installers, two independent jobs update package indexes:

- Homebrew updates `Casks/doze.rb` in `fayeed/homebrew-tap`. Configure the repository secret `HOMEBREW_TAP_TOKEN` with write access to that tap. Install with `brew install --cask fayeed/tap/doze`.
- WinGet submits the versioned `Fayeed.Doze` manifest to Microsoft's community repository. Configure `WINGET_SUBMISSION_TOKEN` with permission to open submissions to `microsoft/winget-pkgs`. Microsoft validates and reviews submissions before users can install with `winget install --id Fayeed.Doze --exact`.

Both manifests use checksums from the release artifacts and immutable tag URLs. Distribution jobs run independently after the GitHub Release publishes; rerun a failed package job without rebuilding installers. WinGet availability depends on Microsoft's review and catalog processing.

## Local builds and uploads

On Windows or macOS, `pnpm release --build-only` builds that machine's installer without publishing it. To upload a build manually, authenticate the GitHub CLI with repository write access and run `pnpm release v0.1.0`; the matching GitHub Release must already exist. The automated tag workflow is recommended because it publishes both platform assets together.

`pnpm release:test` checks release planning and installer selection. The workflow builds desktop installers only; it does not build an iOS app or configure automatic in-app updates.

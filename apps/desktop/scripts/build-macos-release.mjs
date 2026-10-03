import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const appDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

if (process.platform !== "darwin") {
  console.error("The notarized macOS release build must run on macOS.");
  process.exit(1);
}

if (!process.env.APPLE_SIGNING_IDENTITY) {
  console.error(
    "Set APPLE_SIGNING_IDENTITY to your Developer ID Application certificate name.",
  );
  process.exit(1);
}

const hasApiCredentials =
  process.env.APPLE_API_ISSUER &&
  process.env.APPLE_API_KEY &&
  process.env.APPLE_API_KEY_PATH;
const hasAppleIdCredentials =
  process.env.APPLE_ID &&
  process.env.APPLE_PASSWORD &&
  process.env.APPLE_TEAM_ID;

if (!hasApiCredentials && !hasAppleIdCredentials) {
  console.error(
    "Set App Store Connect credentials (APPLE_API_ISSUER, APPLE_API_KEY, APPLE_API_KEY_PATH) " +
      "or Apple ID credentials (APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID) for notarization.",
  );
  process.exit(1);
}

const tauriCli = path.join(appDir, "node_modules/@tauri-apps/cli/tauri.js");
const result = spawnSync(
  process.execPath,
  [tauriCli, "build", "--bundles", "app,dmg", ...process.argv.slice(2)],
  { cwd: appDir, env: process.env, stdio: "inherit" },
);

if (result.error) {
  console.error(
    `Could not start the Tauri release build: ${result.error.message}`,
  );
  process.exit(1);
}

process.exit(result.status ?? 1);

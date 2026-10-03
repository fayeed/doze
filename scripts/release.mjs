import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const desktop = path.join(root, "apps/desktop");

export function releasePlan(platform, version) {
  if (!/^\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(version))
    throw new Error("Invalid release version.");
  if (platform === "win32")
    return {
      platform: "windows",
      filename: "Doze-windows-x64-setup.exe",
      buildArgs: ["build", "--bundles", "nsis", "--target", "x86_64-pc-windows-msvc"],
      bundleDir: "x86_64-pc-windows-msvc/release/bundle/nsis",
      extension: ".exe",
      version,
    };
  if (platform === "darwin")
    return {
      platform: "macos",
      filename: "Doze-macos-universal.dmg",
      buildArgs: ["build", "--bundles", "app,dmg", "--target", "universal-apple-darwin"],
      bundleDir: "universal-apple-darwin/release/bundle/dmg",
      extension: ".dmg",
      version,
    };
  throw new Error("Run releases on Windows or macOS.");
}

export async function findInstaller(folder, plan, startedAt) {
  const names = (await readdir(folder)).filter(
    (name) => name.endsWith(plan.extension) && name.includes(`_${plan.version}_`),
  );
  const candidates = [];
  for (const name of names) {
    const file = path.join(folder, name);
    const info = await stat(file);
    if (info.isFile() && info.size > 0 && info.mtimeMs >= startedAt - 2000)
      candidates.push(file);
  }
  if (candidates.length !== 1)
    throw new Error(`Expected one freshly built ${plan.extension} for ${plan.version}; found ${candidates.length}.`);
  return candidates[0];
}

function run(command, args, cwd = root, capture = false) {
  const result = spawnSync(command, args, {
    cwd,
    env: process.env,
    stdio: capture ? "pipe" : "inherit",
    encoding: "utf8",
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`${path.basename(command)} failed (${result.status}). ${capture ? result.stderr : "See output above."}`);
  return result.stdout;
}

async function main() {
  const args = process.argv.slice(2);
  if (args.includes("--help")) {
    console.log("pnpm release [vVERSION] [--build-only]\nBuild this OS's installer. Supply a GitHub release tag to upload it with gh; tag pushes in GitHub Actions build and publish both platforms together.");
    return;
  }
  const buildOnly = args.includes("--build-only");
  const rest = args.filter((arg) => arg !== "--build-only");
  if (rest.length > 1) throw new Error("Usage: pnpm release [vVERSION] [--build-only].");
  const envFile = path.join(root, ".env.release");
  if (existsSync(envFile)) process.loadEnvFile(envFile);
  if (process.env.CARGO_TARGET_DIR || process.env.CARGO_BUILD_TARGET)
    throw new Error("Unset CARGO_TARGET_DIR and CARGO_BUILD_TARGET for release builds.");
  const { version } = JSON.parse(await readFile(path.join(desktop, "src-tauri/tauri.conf.json"), "utf8"));
  const plan = releasePlan(process.platform, version);
  const tag = rest[0];
  if (process.env.GITHUB_REF_NAME && process.env.GITHUB_REF_NAME !== `v${version}`)
    throw new Error(`Git tag ${process.env.GITHUB_REF_NAME} does not match app version v${version}.`);
  if (tag && tag !== `v${version}`)
    throw new Error(`Release tag ${tag} does not match app version v${version}.`);
  if (!buildOnly && !tag)
    throw new Error(`Supply the GitHub release tag v${version}, or use --build-only.`);
  if (process.platform === "darwin" && !buildOnly) {
    const hasApi = process.env.APPLE_API_ISSUER && process.env.APPLE_API_KEY && process.env.APPLE_API_KEY_PATH;
    const hasAppleId = process.env.APPLE_ID && process.env.APPLE_PASSWORD && process.env.APPLE_TEAM_ID;
    if (!process.env.APPLE_SIGNING_IDENTITY || (!hasApi && !hasAppleId))
      throw new Error("macOS releases require signing and Apple notarization credentials. See docs/releases.md.");
  }
  if (!buildOnly) run("gh", ["release", "view", tag]);
  console.log(`Release ${tag || `v${version}`}: ${plan.platform}\nBuild: tauri ${plan.buildArgs.join(" ")}`);
  const buildStartedAt = Date.now();
  if (process.platform === "darwin")
    run(process.execPath, [path.join(desktop, "scripts/build-macos-release.mjs"), ...plan.buildArgs.slice(3)], desktop);
  else {
    const rustInfo = run("rustc", ["-vV"], desktop, true);
    if (!rustInfo.includes("host: x86_64-pc-windows-msvc"))
      throw new Error("Windows releases require the x86_64-pc-windows-msvc Rust toolchain.");
    run(process.execPath, [path.join(desktop, "node_modules/@tauri-apps/cli/tauri.js"), ...plan.buildArgs], desktop);
  }
  const file = await findInstaller(path.join(desktop, "src-tauri/target", plan.bundleDir), plan, buildStartedAt);
  const publishedFile = path.join(path.dirname(file), plan.filename);
  const { copyFile } = await import("node:fs/promises");
  await copyFile(file, publishedFile);
  if (buildOnly) {
    console.log(`Built: ${publishedFile}`);
    return;
  }
  run("gh", ["release", "upload", tag, publishedFile, "--clobber"]);
  console.log(`Uploaded ${plan.filename} to ${tag}.`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href)
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });

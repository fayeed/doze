import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream, existsSync } from "node:fs";
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
      buildArgs: [
        "build",
        "--bundles",
        "nsis",
        "--target",
        "x86_64-pc-windows-msvc",
      ],
      bundleDir: "x86_64-pc-windows-msvc/release/bundle/nsis",
      extension: ".exe",
      version,
    };
  if (platform === "darwin")
    return {
      platform: "macos",
      filename: "Doze-macos-universal.dmg",
      buildArgs: [
        "build",
        "--bundles",
        "app,dmg",
        "--target",
        "universal-apple-darwin",
      ],
      bundleDir: "universal-apple-darwin/release/bundle/dmg",
      extension: ".dmg",
      version,
    };
  throw new Error("Run releases on Windows or macOS.");
}

export function releaseSettings(config, env = process.env) {
  const bucket = env.DOZE_R2_BUCKET || config.bucket;
  const base = env.DOZE_DOWNLOAD_BASE_URL || config.publicBaseUrl;
  if (!bucket || !base)
    throw new Error(
      "Set bucket and publicBaseUrl in release.config.json (or DOZE_R2_BUCKET and DOZE_DOWNLOAD_BASE_URL).",
    );
  if (!/^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/.test(bucket))
    throw new Error("Invalid R2 bucket name.");
  const url = new URL(base);
  if (
    url.protocol !== "https:" ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    url.pathname !== "/"
  ) {
    throw new Error(
      "publicBaseUrl must be the HTTPS root of your public R2 bucket, without credentials, a path, query or fragment.",
    );
  }
  return { bucket, base: url.origin };
}

export async function findInstaller(folder, plan, startedAt) {
  const names = (await readdir(folder)).filter(
    (name) =>
      name.endsWith(plan.extension) && name.includes(`_${plan.version}_`),
  );
  const candidates = [];
  for (const name of names) {
    const file = path.join(folder, name);
    const info = await stat(file);
    if (info.isFile() && info.size > 0 && info.mtimeMs >= startedAt - 2000)
      candidates.push(file);
  }
  if (candidates.length !== 1)
    throw new Error(
      `Expected one freshly built ${plan.extension} for ${plan.version} in ${folder}; found ${candidates.length}. Nothing uploaded.`,
    );
  return candidates[0];
}

export function objectKeys(plan, hash) {
  return [
    `releases/${plan.version}/${plan.platform}/${hash}/${plan.filename}`,
    `releases/latest/${plan.filename}`,
  ];
}

function run(command, args, cwd = root, capture = false) {
  const result = spawnSync(command, args, {
    cwd,
    env: { ...process.env, WRANGLER_SEND_METRICS: "false" },
    stdio: capture ? "pipe" : "inherit",
    encoding: "utf8",
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(
      `${path.basename(command)} failed (${result.status}). ${capture ? result.stderr : "See output above."}`,
    );
  return result.stdout;
}

export async function publishInstaller({
  plan,
  file,
  settings,
  upload,
  verify,
}) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  const digest = hash.digest("hex");
  const [archive, latest] = objectKeys(plan, digest);
  // Verify the archived download before replacing the public latest installer.
  await upload(archive, "public, max-age=31536000, immutable");
  await verify(`${settings.base}/${archive}`, digest);
  await upload(latest, "no-store");
  await verify(`${settings.base}/${latest}`, digest);
  return { url: `${settings.base}/${latest}`, sha256: digest };
}

async function verifyDownload(url, expected) {
  const response = await fetch(url, {
    cache: "no-store",
    signal: AbortSignal.timeout(300_000),
  });
  if (!response.ok || !response.body)
    throw new Error(
      `Public download verification failed (${response.status}): ${url}`,
    );
  const hash = createHash("sha256");
  for await (const chunk of response.body) hash.update(chunk);
  if (hash.digest("hex") !== expected)
    throw new Error(
      `Downloaded file does not match the build: ${url}. Check public access and Cloudflare cache rules.`,
    );
}

async function main() {
  const args = process.argv.slice(2);
  if (args.includes("--help")) {
    console.log(
      "pnpm release [--dry-run]\nBuild and publish this OS to Cloudflare R2. Configure release.config.json; use wrangler login or CLOUDFLARE_API_TOKEN. Optional secrets: .env.release (ignored by Git).",
    );
    return;
  }
  if (args.some((arg) => arg !== "--dry-run"))
    throw new Error("Unknown option. Use --help.");
  const envFile = path.join(root, ".env.release");
  if (existsSync(envFile)) process.loadEnvFile(envFile);
  if (process.env.CARGO_TARGET_DIR || process.env.CARGO_BUILD_TARGET)
    throw new Error(
      "Unset CARGO_TARGET_DIR and CARGO_BUILD_TARGET for release builds; native companions use the repository's standard output paths.",
    );
  const config = JSON.parse(
    await readFile(path.join(root, "release.config.json"), "utf8"),
  );
  const settings = releaseSettings(config);
  const { version } = JSON.parse(
    await readFile(path.join(desktop, "src-tauri/tauri.conf.json"), "utf8"),
  );
  const plan = releasePlan(process.platform, version);
  if (process.platform === "darwin" && !args.includes("--dry-run")) {
    const hasApiCredentials =
      process.env.APPLE_API_ISSUER &&
      process.env.APPLE_API_KEY &&
      process.env.APPLE_API_KEY_PATH;
    const hasAppleIdCredentials =
      process.env.APPLE_ID &&
      process.env.APPLE_PASSWORD &&
      process.env.APPLE_TEAM_ID;
    if (
      !process.env.APPLE_SIGNING_IDENTITY ||
      (!hasApiCredentials && !hasAppleIdCredentials)
    ) {
      throw new Error(
        "macOS releases require APPLE_SIGNING_IDENTITY and Apple notarization credentials. See docs/releases.md.",
      );
    }
  }
  console.log(
    `Release ${version}: ${plan.platform}\nBuild: tauri ${plan.buildArgs.join(" ")}\nBucket: ${settings.bucket}\nDownload: ${settings.base}/releases/latest/${plan.filename}`,
  );
  if (args.includes("--dry-run")) return;
  const wrangler = path.join(root, "node_modules/wrangler/bin/wrangler.js");
  // Check authentication and bucket access before spending time on the build.
  run(process.execPath, [wrangler, "r2", "bucket", "info", settings.bucket]);
  if (process.platform === "darwin")
    run("rustup", [
      "target",
      "add",
      "aarch64-apple-darwin",
      "x86_64-apple-darwin",
    ]);
  else {
    const rustInfo = run("rustc", ["-vV"], desktop, true);
    if (!rustInfo.includes("host: x86_64-pc-windows-msvc"))
      throw new Error(
        "Windows releases require the x86_64-pc-windows-msvc Rust toolchain (including the native CLI companion).",
      );
  }
  const startedAt = Date.now();
  if (process.platform === "darwin")
    run(
      process.execPath,
      [
        path.join(desktop, "scripts/build-macos-release.mjs"),
        ...plan.buildArgs.slice(3),
      ],
      desktop,
    );
  else
    run(
      process.execPath,
      [
        path.join(desktop, "node_modules/@tauri-apps/cli/tauri.js"),
        ...plan.buildArgs,
      ],
      desktop,
    );
  const file = await findInstaller(
    path.join(desktop, "src-tauri/target", plan.bundleDir),
    plan,
    startedAt,
  );
  // Wrangler's object upload limit is 315 MB.
  if ((await stat(file)).size > 315_000_000)
    throw new Error(
      "Installer exceeds Wrangler's 315 MB upload limit; use R2 multipart upload for this release.",
    );
  const result = await publishInstaller({
    plan,
    file,
    settings,
    verify: verifyDownload,
    upload: (key, cacheControl) =>
      run(process.execPath, [
        wrangler,
        "r2",
        "object",
        "put",
        `${settings.bucket}/${key}`,
        "--remote",
        "--file",
        file,
        "--content-type",
        "application/octet-stream",
        "--content-disposition",
        `attachment; filename="${plan.filename}"`,
        "--cache-control",
        cacheControl,
      ]),
  });
  console.log(
    `Published and verified: ${result.url}\nSHA-256: ${result.sha256}`,
  );
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}

import { existsSync, mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const desktop = fileURLToPath(new URL("../", import.meta.url));
if (process.platform === "darwin") {
  const sdk = spawnSync("xcrun", ["--sdk", "macosx", "--show-sdk-version"], {
    encoding: "utf8",
  });
  if (sdk.status !== 0 || Number.parseInt(sdk.stdout, 10) < 26) {
    console.error(
      "Xcode with a macOS 26 or newer SDK is required for native Liquid Glass.",
    );
    process.exit(1);
  }
  const output = path.join(desktop, "native/macos/publish");
  mkdirSync(output, { recursive: true });
  const slices = [];
  for (const arch of ["arm64", "x86_64"]) {
    const slice = path.join(output, `Doze.NativeUI-${arch}`);
    const build = spawnSync(
      "xcrun",
      [
        "swiftc",
        "-swift-version",
        "5",
        "-O",
        "-parse-as-library",
        "-target",
        `${arch}-apple-macos13.0`,
        "native/macos/Doze.swift",
        "-o",
        slice,
      ],
      { cwd: desktop, stdio: "inherit" },
    );
    if (build.error) console.error(build.error.message);
    if (build.status !== 0) process.exit(build.status ?? 1);
    slices.push(slice);
  }
  const executable = path.join(output, "Doze.NativeUI");
  const merged = spawnSync(
    "xcrun",
    ["lipo", "-create", ...slices, "-output", executable],
    { stdio: "inherit" },
  );
  if (merged.status !== 0) process.exit(merged.status ?? 1);
  // Developer builds need a valid local signature after combining the architecture slices.
  const signed = spawnSync("codesign", ["--force", "--sign", "-", executable], {
    stdio: "inherit",
  });
  process.exit(signed.status ?? 1);
}
if (process.platform !== "win32") process.exit(0);

// doze.exe is a GUI program, so shells neither wait for it nor keep its exit status.
// doze-cli.exe is the console front end for `doze run` and `doze watch`, installed beside it.
const cli = spawnSync(
  "cargo",
  [
    "build",
    "--release",
    "--locked",
    "--manifest-path",
    "native/cli/Cargo.toml",
  ],
  { cwd: desktop, stdio: "inherit" },
);
if (cli.error) console.error(cli.error.message);
if (cli.status !== 0) process.exit(cli.status ?? 1);

const localSdk = path.resolve(desktop, "../../.tools/dotnet/dotnet.exe");
const dotnet = existsSync(localSdk) ? localSdk : "dotnet";
const result = spawnSync(
  dotnet,
  [
    "publish",
    "native/windows/Doze.Settings.csproj",
    "-c",
    "Release",
    "-p:Platform=x64",
    "-o",
    "native/windows/publish",
  ],
  {
    cwd: desktop,
    stdio: "inherit",
    env: { ...process.env, DOTNET_CLI_TELEMETRY_OPTOUT: "1" },
  },
);
if (result.error) {
  console.error(
    "A .NET 8 SDK is required to build the native WinUI Settings companion.",
    result.error.message,
  );
  process.exit(1);
}
process.exit(result.status ?? 1);

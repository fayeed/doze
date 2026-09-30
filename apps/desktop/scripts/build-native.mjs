import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

if (process.platform !== "win32") process.exit(0);

const desktop = fileURLToPath(new URL("../", import.meta.url));
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

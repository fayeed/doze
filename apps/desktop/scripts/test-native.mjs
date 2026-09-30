import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const desktop = fileURLToPath(new URL("../", import.meta.url));
const command =
  process.platform === "darwin"
    ? "native/macos/publish/Doze.NativeUI"
    : "powershell";
const args =
  process.platform === "darwin"
    ? ["--verify-ui"]
    : [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "scripts/test-native.ps1",
      ];
const result = spawnSync(command, args, {
  cwd: desktop,
  stdio: "inherit",
  timeout: 30000,
});
if (result.error) console.error(result.error.message);
process.exit(result.status ?? 1);

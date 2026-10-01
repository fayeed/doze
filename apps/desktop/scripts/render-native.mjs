import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

// Exports every native page and window in light and dark to PNG files for visual review.
// Uses sample data; it never contacts the engine, saves preferences or performs power actions.
const desktop = fileURLToPath(new URL("../", import.meta.url));
if (process.platform !== "darwin") {
  console.error(
    "native:render exports the macOS companion. Windows renders use native:test.",
  );
  process.exit(1);
}
const output = path.resolve(
  process.argv[2] ?? path.join(desktop, "native/macos/renders"),
);
const result = spawnSync(
  "native/macos/publish/Doze.NativeUI",
  ["--render-ui", output],
  { cwd: desktop, stdio: "inherit", timeout: 60000 },
);
if (result.error) console.error(result.error.message);
if (result.status === 0) console.log(`Rendered native pages to ${output}`);
process.exit(result.status ?? 1);

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const expected = "0.2.0";
const jsonFiles = ["package.json", "apps/desktop/package.json", "apps/web/package.json", "packages/brand/package.json", "packages/config/package.json", "apps/desktop/src-tauri/tauri.conf.json"];
for (const file of jsonFiles) {
  const manifest = JSON.parse(await readFile(file, "utf8"));
  assert.equal(manifest.version, expected, `${file} must be ${expected}`);
}
for (const file of ["apps/desktop/src-tauri/Cargo.toml", "apps/desktop/native/cli/Cargo.toml"]) {
  const manifest = await readFile(file, "utf8");
  assert.match(manifest, new RegExp(`^version = "${expected.replaceAll(".", "\\.")}"$`, "m"), `${file} must be ${expected}`);
}
const appLock = await readFile("apps/desktop/src-tauri/Cargo.lock", "utf8");
assert.match(appLock, new RegExp(`name = "doze"\\nversion = "${expected.replaceAll(".", "\\.")}"`), "Rust application lock must be current");
const cliLock = await readFile("apps/desktop/native/cli/Cargo.lock", "utf8");
assert.match(cliLock, new RegExp(`name = "doze-cli"\\nversion = "${expected.replaceAll(".", "\\.")}"`), "Rust CLI lock must be current");
console.log(`All application manifests use ${expected}.`);

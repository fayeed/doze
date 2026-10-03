import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, utimes } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { findInstaller, releasePlan } from "./release.mjs";

test("release plans target the supported Windows and universal macOS builds", () => {
  const win = releasePlan("win32", "0.1.0");
  const mac = releasePlan("darwin", "0.1.0");
  assert.ok(win.buildArgs.includes("x86_64-pc-windows-msvc"));
  assert.ok(mac.buildArgs.includes("universal-apple-darwin"));
  assert.equal(win.filename, "Doze-windows-x64-setup.exe");
  assert.equal(mac.filename, "Doze-macos-universal.dmg");
  assert.throws(() => releasePlan("linux", "0.1.0"));
});

test("stale, wrong-version and ambiguous installers are rejected", async (t) => {
  const folder = await mkdtemp(path.join(os.tmpdir(), "doze-release-test-"));
  t.after(() => rm(folder, { recursive: true, force: true }));
  const plan = releasePlan("win32", "0.1.0");
  const file = path.join(folder, "Doze_0.1.0_x64-setup.exe");
  await writeFile(file, "installer");
  await utimes(file, new Date(0), new Date(0));
  await writeFile(path.join(folder, "Doze_0.2.0_x64-setup.exe"), "wrong version");
  await assert.rejects(findInstaller(folder, plan, Date.now()), /found 0/);
  await utimes(file, new Date(), new Date());
  assert.equal(await findInstaller(folder, plan, Date.now()), file);
  await writeFile(path.join(folder, "Doze_0.1.0_other.exe"), "ambiguous");
  await assert.rejects(findInstaller(folder, plan, Date.now()), /found 2/);
});

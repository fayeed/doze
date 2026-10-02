import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, utimes } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  findInstaller,
  objectKeys,
  publishInstaller,
  releasePlan,
  releaseSettings,
} from "./release.mjs";

test("Windows and universal Mac releases have distinct stable keys", () => {
  const win = releasePlan("win32", "0.1.0");
  const mac = releasePlan("darwin", "0.1.0");
  assert.ok(win.buildArgs.includes("x86_64-pc-windows-msvc"));
  assert.ok(mac.buildArgs.includes("universal-apple-darwin"));
  assert.equal(
    objectKeys(win, "abc")[1],
    "releases/latest/Doze-windows-x64-setup.exe",
  );
  assert.equal(
    objectKeys(mac, "abc")[1],
    "releases/latest/Doze-macos-universal.dmg",
  );
  assert.throws(() => releasePlan("linux", "0.1.0"));
});

test("configuration requires a bucket and a public HTTPS origin", () => {
  assert.throws(() => releaseSettings({}, {}));
  for (const publicBaseUrl of [
    "http://example.com",
    "https://user:password@example.com",
    "https://example.com/private",
    "https://example.com/?token=secret",
  ]) {
    assert.throws(() =>
      releaseSettings({ bucket: "doze-releases", publicBaseUrl }, {}),
    );
  }
  assert.deepEqual(
    releaseSettings(
      {},
      {
        DOZE_R2_BUCKET: "doze-releases",
        DOZE_DOWNLOAD_BASE_URL: "https://example.com/",
      },
    ),
    { bucket: "doze-releases", base: "https://example.com" },
  );
});

test("stale, wrong-version, and ambiguous installers cannot be published", async (t) => {
  const folder = await mkdtemp(path.join(os.tmpdir(), "doze-release-test-"));
  t.after(() => rm(folder, { recursive: true, force: true }));
  const plan = releasePlan("win32", "0.1.0");
  const file = path.join(folder, "Doze_0.1.0_x64-setup.exe");
  await writeFile(file, "installer");
  await utimes(file, new Date(0), new Date(0));
  await writeFile(
    path.join(folder, "Doze_0.2.0_x64-setup.exe"),
    "wrong version",
  );
  await assert.rejects(findInstaller(folder, plan, Date.now()), /found 0/);
  await utimes(file, new Date(), new Date());
  assert.equal(await findInstaller(folder, plan, Date.now()), file);
  await writeFile(path.join(folder, "Doze_0.1.0_other.exe"), "ambiguous");
  await assert.rejects(findInstaller(folder, plan, Date.now()), /found 2/);
});

test("latest changes only after archive upload and public verification succeed", async (t) => {
  const folder = await mkdtemp(path.join(os.tmpdir(), "doze-publish-test-"));
  t.after(() => rm(folder, { recursive: true, force: true }));
  const file = path.join(folder, "installer.exe");
  await writeFile(file, "installer");
  const options = {
    plan: releasePlan("win32", "0.1.0"),
    file,
    settings: { base: "https://example.com" },
  };
  const events = [];
  const result = await publishInstaller({
    ...options,
    upload: async (key, cache) => events.push(["upload", key, cache]),
    verify: async (url) => events.push(["verify", url]),
  });
  assert.deepEqual(
    events.map((event) => event[0]),
    ["upload", "verify", "upload", "verify"],
  );
  assert.equal(events[2][1], "releases/latest/Doze-windows-x64-setup.exe");
  assert.equal(events[2][2], "no-store");
  assert.match(result.sha256, /^[a-f0-9]{64}$/);
  for (const failure of ["upload", "verify"]) {
    const uploaded = [];
    await assert.rejects(
      publishInstaller({
        ...options,
        upload: async (key) => {
          uploaded.push(key);
          if (failure === "upload") throw new Error("failed upload");
        },
        verify: async () => {
          throw new Error("failed verification");
        },
      }),
      /failed/,
    );
    assert.equal(uploaded.length, 1);
    assert.ok(!uploaded[0].includes("/latest/"));
  }
});

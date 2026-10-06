import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { once } from "node:events";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const desktop = fileURLToPath(new URL("../", import.meta.url));
// Release mode allows integration tests while the debug desktop app is running.
const release = process.env.DOZE_MCP_TEST_RELEASE === "1";
const build = spawnSync(
  "cargo",
  [
    "build",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "--locked",
    "--features",
    "mcp-test-support",
    "--bins",
    ...(release ? ["--release"] : []),
  ],
  { cwd: desktop, stdio: "inherit" },
);
assert.equal(build.status, 0, "Build failed");
const suffix = process.platform === "win32" ? ".exe" : "";
const binaries = path.join(
  desktop,
  "src-tauri/target",
  release ? "release" : "debug",
);
const directory = await mkdtemp(path.join(tmpdir(), "doze-mcp-"));
const endpoint = path.join(directory, "endpoint.json");
// This host contains only MockPower; the actual application binary is used for stdio.
const host = spawn(
  path.join(binaries, `doze-mcp-test-host${suffix}`),
  [endpoint],
  { stdio: ["pipe", "pipe", "inherit"], windowsHide: true },
);
const lines = createInterface({ input: host.stdout });
const output = lines[Symbol.asyncIterator]();
const clients = [];
const guard = setTimeout(() => {
  host.kill();
  process.exitCode = 1;
}, 60000);
const command = async (name) => {
  host.stdin.write(`${name}\n`);
  return JSON.parse((await output.next()).value);
};
const connect = async (
  name,
  key = `${name}-${"x".repeat(32)}`,
  extraEnv = {},
) => {
  const client = new Client({ name, version: "1.0.0" });
  const transport = new StdioClientTransport({
    command: path.join(binaries, `doze${suffix}`),
    args: ["--mcp", "--endpoint", endpoint],
    env: { ...process.env, DOZE_MCP_KEY: key, ...extraEnv },
    stderr: "inherit",
  });
  await client.connect(transport);
  clients.push(client);
  return client;
};
const call = async (client, name, args = {}) => {
  const result = await client.callTool({
    name: `doze.${name}`,
    arguments: args,
  });
  assert.notEqual(result.isError, true, JSON.stringify(result));
  return result.structuredContent.result;
};
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
// `doze run` from a terminal: the same binary, talking to the engine as a command-line job.
const job = (args) =>
  new Promise((resolve) => {
    const child = spawn(
      path.join(binaries, `doze${suffix}`),
      ["run", "--endpoint", endpoint, ...args],
      { stdio: ["ignore", "ignore", "pipe"], windowsHide: true },
    );
    let stderr = "";
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("exit", (code) => resolve({ code, stderr }));
  });
try {
  assert.equal((await output.next()).value, "ready");
  const shell =
    process.platform === "win32" ? ["cmd", "/c"] : ["/bin/sh", "-c"];
  const succeeded = await job(["--then", "sleep", "--", ...shell, "exit 0"]);
  assert.equal(succeeded.code, 0, succeeded.stderr);
  assert.match(succeeded.stderr, /Doze will sleep after its final warning/);
  assert.deepEqual(await command("state"), {
    awake: true,
    countdown: true,
    executed: 0,
  });
  await command("cancel");
  const failed = await job(["--then", "sleep", "--", ...shell, "exit 3"]);
  assert.equal(failed.code, 3, failed.stderr);
  assert.match(failed.stderr, /Doze will not sleep/);
  assert.deepEqual(await command("state"), {
    awake: false,
    countdown: false,
    executed: 0,
  });
  // The bridge keeps a connected agent's session alive through a long step.
  const patient = await connect("claude", undefined, {
    DOZE_KEEPALIVE_INTERVAL_SECONDS: "1",
  });
  const long = await call(patient, "start_session", { reason: "Long build" });
  await pause(3500);
  const kept = await call(patient, "get_session", {
    session_id: long.session_id,
  });
  assert(kept.last_heartbeat > long.last_heartbeat, "keep-alive renewed");
  assert.equal(kept.status, "active");
  await patient.close();
  const observer = await connect("claude");
  const before = await call(observer, "get_session", {
    session_id: long.session_id,
  });
  await pause(2500);
  const after = await call(observer, "get_session", {
    session_id: long.session_id,
  });
  assert.equal(after.last_heartbeat, before.last_heartbeat, "renewal stops");
  await call(observer, "cancel_session", { session_id: long.session_id });
  const codex = await connect("codex");
  const claude = await connect("claude");
  const listed = await codex.listTools();
  assert.equal(listed.tools.length, 8);
  assert(
    !listed.tools.some((t) =>
      /sleep_now|shutdown_now|execute_command/.test(t.name),
    ),
  );
  const a = await call(codex, "start_session", {
    reason: "Refactor and test",
    completion_action: "sleep",
  });
  assert.equal(a.status, "active");
  assert.equal(a.test_awake, true);
  // Two command-line jobs and the keep-alive session above remain in the history.
  assert.equal(a.test_ui_sessions, 4);
  const renewed = await call(codex, "heartbeat", { session_id: a.session_id });
  assert(renewed.lease_expires_at > a.lease_expires_at);
  const b = await call(claude, "start_session", {
    reason: "Run tests",
    completion_action: "sleep",
  });
  const first = await call(codex, "finish_session", {
    session_id: a.session_id,
  });
  assert.equal(first.test_awake, true);
  assert.equal(first.test_countdown, false);
  const last = await call(claude, "finish_session", {
    session_id: b.session_id,
  });
  assert.equal(last.test_countdown, true);
  assert.equal(last.test_executed, 0);
  const cancelled = await command("cancel");
  assert.equal(cancelled.countdown, false);
  const read = await call(claude, "get_session", { session_id: b.session_id });
  assert.equal(read.test_countdown, false);
  const unauthorized = await call(codex, "start_session", {
    reason: "Unsafe request",
    completion_action: "shutdown",
  });
  assert.equal(unauthorized.status, "awaiting_authorization");
  assert.equal(unauthorized.test_awake, false);
  const badFinish = await codex.callTool({
    name: "doze.finish_session",
    arguments: { session_id: unauthorized.session_id },
  });
  assert.equal(badFinish.isError, true);
  await call(codex, "cancel_session", { session_id: unauthorized.session_id });
  const cross = await claude.callTool({
    name: "doze.get_session",
    arguments: { session_id: a.session_id },
  });
  assert.equal(cross.isError, true);
  const stranger = await connect("stranger", "invalid");
  assert.equal(
    (
      await stranger.callTool({
        name: "doze.start_session",
        arguments: { reason: "Spoof" },
      })
    ).isError,
    true,
  );
  const lost = await call(codex, "start_session", {
    reason: "Interrupted work",
    completion_action: "sleep",
  });
  await codex.close();
  assert.deepEqual(await command("expire"), {
    awake: true,
    countdown: false,
    executed: 0,
  });
  const reconnected = await connect("codex");
  assert.equal(
    (await call(reconnected, "get_session", { session_id: lost.session_id }))
      .status,
    "connection_lost",
  );
  await call(reconnected, "cancel_session", { session_id: lost.session_id });
  const normal = await call(reconnected, "start_session", {
    reason: "Return normally",
    completion_action: "return_to_normal",
  });
  const sleepy = await call(claude, "start_session", {
    reason: "Sleep request",
    completion_action: "sleep",
  });
  await call(reconnected, "finish_session", { session_id: normal.session_id });
  const conflict = await call(claude, "finish_session", {
    session_id: sleepy.session_id,
  });
  assert.equal(conflict.test_countdown, false);
  assert.equal(conflict.test_awake, false);
  const successful = await call(reconnected, "start_session", {
    reason: "Third agent",
    completion_action: "sleep",
  });
  const failedPeer = await call(claude, "start_session", {
    reason: "Model unavailable",
    completion_action: "sleep",
  });
  const failure = await call(claude, "fail_session", {
    session_id: failedPeer.session_id,
  });
  assert.equal(failure.status, "failed");
  assert.equal(failure.test_countdown, false);
  const successfulFinish = await call(reconnected, "finish_session", {
    session_id: successful.session_id,
  });
  assert.equal(successfulFinish.test_countdown, true);
  await command("cancel");
  // A valid bounded history can exceed the request-size limit; responses use a
  // separate limit so list_sessions continues to work for long overlapping batches.
  const anchor = await call(reconnected, "start_session", {
    reason: "Keep batch open",
    completion_action: "return_to_normal",
  });
  for (let index = 0; index < 90; index++) {
    const item = await call(reconnected, "start_session", {
      reason: "x".repeat(512),
      completion_action: "return_to_normal",
    });
    await call(reconnected, "finish_session", { session_id: item.session_id });
  }
  const history = await call(reconnected, "list_sessions");
  assert(JSON.stringify(history).length > 64 * 1024);
  assert(history.some((item) => item.session_id === anchor.session_id));
  await call(reconnected, "cancel_session", { session_id: anchor.session_id });
  console.log(
    "MCP SDK verified: command-line jobs (success, failure), bridge keep-alive while connected, real stdio handshake, tools, mock wake acquisition, UI snapshot, heartbeat, finish/countdown, cancellation, multiple clients, conflicting actions, unauthorized actions, ownership, disconnect/expiry and reconnect. No native power actions.",
  );
} finally {
  clearTimeout(guard);
  await Promise.allSettled(clients.map((client) => client.close()));
  lines.close();
  host.kill();
  if (host.exitCode === null) await once(host, "exit");
  await rm(directory, { recursive: true, force: true });
}

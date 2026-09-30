import test from "node:test";
import assert from "node:assert/strict";
import { watchDozeRun } from "./runtime-lease.mjs";
function client(initial = "active") {
  const calls = [];
  let status = initial;
  return {
    calls,
    async callTool(request) {
      calls.push(request.name);
      if (request.name === "doze.get_session") status = "active";
      return {
        structuredContent: {
          result: {
            session_id: "test",
            status,
            last_heartbeat: 0,
            lease_expires_at: 30,
          },
        },
      };
    },
  };
}
test("runtime renews without model calls and finishes only on explicit success", async () => {
  const mock = client();
  const states = ["running", "running", "succeeded"];
  const result = await watchDozeRun(
    mock,
    { reason: "test" },
    { readStatus: () => states.shift(), pollIntervalMs: 1 },
  );
  assert.equal(result.outcome, "finished");
  assert.deepEqual(mock.calls, [
    "doze.start_session",
    "doze.heartbeat",
    "doze.heartbeat",
    "doze.finish_session",
  ]);
});
test("uncertainty or disconnection stops renewals without finish or cancel", async () => {
  for (const status of [
    "disconnected",
    "waiting_for_input",
    "cancelled",
    undefined,
  ]) {
    const mock = client();
    const result = await watchDozeRun(
      mock,
      { reason: "test" },
      { readStatus: () => status },
    );
    assert.equal(result.outcome, "uncertain");
    assert.deepEqual(mock.calls, ["doze.start_session"]);
  }
});
test("hanging runtime status cannot renew indefinitely", async () => {
  const mock = client();
  const result = await watchDozeRun(
    mock,
    { reason: "test" },
    { readStatus: () => new Promise(() => {}), observationTimeoutMs: 5 },
  );
  assert.equal(result.outcome, "uncertain");
  assert.equal(mock.calls.length, 1);
});
test("authorization wait never renews an unapproved lease", async () => {
  const mock = client("awaiting_authorization");
  let count = 0;
  const result = await watchDozeRun(
    mock,
    { reason: "test" },
    {
      readStatus: () => (count++ ? "succeeded" : "running"),
      pollIntervalMs: 1,
    },
  );
  assert.equal(result.outcome, "finished");
  assert.deepEqual(mock.calls, [
    "doze.start_session",
    "doze.get_session",
    "doze.finish_session",
  ]);
});
test("runtime abort stops observation and never signals successful completion", async () => {
  const mock = client();
  const controller = new AbortController();
  const result = await watchDozeRun(
    mock,
    { reason: "test" },
    {
      signal: controller.signal,
      readStatus: () => {
        controller.abort();
        return new Promise(() => {});
      },
      observationTimeoutMs: 1000,
    },
  );
  assert.equal(result.outcome, "uncertain");
  assert.equal(mock.calls.length, 1);
});

test("definitive runtime failure explicitly releases its lease without success", async () => {
  const mock = client();
  const result = await watchDozeRun(
    mock,
    { reason: "test" },
    { readStatus: () => "failed" },
  );
  assert.equal(result.outcome, "failed");
  assert.deepEqual(mock.calls, ["doze.start_session", "doze.fail_session"]);
});

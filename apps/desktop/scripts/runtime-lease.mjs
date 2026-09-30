// Reference integration for agent runtimes. No model invocation or process heuristics.
// readStatus must obtain FRESH authoritative run state, including connectivity; a
// cached "running" flag or an open MCP pipe is not sufficient evidence.
const delay = (ms, signal) =>
  new Promise((resolve) => {
    if (signal?.aborted) {
      resolve();
      return;
    }
    const done = () => {
      clearTimeout(timer);
      signal?.removeEventListener("abort", done);
      resolve();
    };
    const timer = setTimeout(done, ms);
    signal?.addEventListener("abort", done, { once: true });
  });
async function tool(client, name, arguments_) {
  const result = await client.callTool({
    name: `doze.${name}`,
    arguments: arguments_,
  });
  if (result.isError)
    throw new Error(result.content?.[0]?.text ?? "Doze request failed");
  return result.structuredContent?.result ?? JSON.parse(result.content[0].text);
}
async function freshStatus(readStatus, timeoutMs, signal) {
  const controller = new AbortController();
  let timer;
  let abort;
  try {
    return await Promise.race([
      Promise.resolve().then(() => readStatus({ signal: controller.signal })),
      new Promise((_, reject) => {
        abort = () => {
          controller.abort();
          reject(new Error("Run observation interrupted"));
        };
        timer = setTimeout(abort, timeoutMs);
        if (signal?.aborted) abort();
        else signal?.addEventListener("abort", abort, { once: true });
      }),
    ]);
  } finally {
    clearTimeout(timer);
    if (abort) signal?.removeEventListener("abort", abort);
    controller.abort();
  }
}
/**
 * Connect an explicit runtime job to Doze. Only "succeeded" finishes the lease.
 * Definitive "failed" releases the lease. Missing status, interruption, and
 * disconnection leave it to expire conservatively. Adapters must interpret success
 * for the whole requested job, including child work and required verification.
 * This is an opt-in integration utility, not automatic Codex/Claude registration.
 */
export async function watchDozeRun(
  client,
  request,
  { readStatus, signal, observationTimeoutMs = 10000, pollIntervalMs } = {},
) {
  if (typeof readStatus !== "function")
    throw new TypeError(
      "A fresh authoritative readStatus callback is required",
    );
  if (signal?.aborted) return { outcome: "not_started" };
  let session = await tool(client, "start_session", request);
  const id = session.session_id;
  const uncertain = (reason) => ({
    outcome: "uncertain",
    reason,
    session_id: id,
  });
  try {
    while (!signal?.aborted) {
      const leaseMs = Math.max(
        1000,
        (session.lease_expires_at - session.last_heartbeat) * 1000,
      );
      const timeoutMs = Math.max(
        1,
        Math.min(observationTimeoutMs, leaseMs / 4),
      );
      const status = await freshStatus(readStatus, timeoutMs, signal);
      if (signal?.aborted) break;
      if (
        status === "failed" &&
        ["active", "connection_lost"].includes(session.status)
      ) {
        await tool(client, "fail_session", { session_id: id });
        return { outcome: "failed", session_id: id };
      }
      if (!["running", "succeeded"].includes(status))
        return uncertain(status ?? "unknown");
      if (session.status === "awaiting_authorization") {
        await delay(Math.min(pollIntervalMs ?? 2000, 2000), signal);
        if (signal?.aborted) break;
        session = await tool(client, "get_session", { session_id: id });
        if (["denied", "cancelled"].includes(session.status))
          return { outcome: session.status, session_id: id };
        continue;
      }
      if (!["active", "connection_lost"].includes(session.status))
        return uncertain(session.status);
      if (status === "succeeded") {
        await tool(client, "finish_session", { session_id: id });
        return { outcome: "finished", session_id: id };
      }
      session = await tool(client, "heartbeat", { session_id: id });
      await delay(
        Math.max(1, Math.min(pollIntervalMs ?? leaseMs / 3, leaseMs / 3)),
        signal,
      );
    }
    return uncertain("interrupted");
  } catch (error) {
    return uncertain(error.message);
  }
}

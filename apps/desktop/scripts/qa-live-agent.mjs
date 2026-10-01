// Interactive hardware QA against a running Doze instance. Uses an existing
// local profile without changing settings or printing its credential.
import { readFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { fileURLToPath } from "node:url";

const action = process.argv[2] ?? "return_to_normal";
if (!["return_to_normal", "sleep"].includes(action)) {
  throw new Error(
    "QA supports return_to_normal or explicitly requested sleep.",
  );
}
// Tauri's per-user configuration directory for app.getdoze.desktop.
const folder =
  process.platform === "win32"
    ? path.join(process.env.APPDATA, "app.getdoze.desktop")
    : path.join(
        os.homedir(),
        "Library/Application Support/app.getdoze.desktop",
      );
const settings = JSON.parse(await readFile(path.join(folder, "settings.json")));
const clientName = process.env.DOZE_QA_CLIENT ?? "Codex";
const profile = settings.agents?.clients?.find(
  (item) => item.name === clientName,
);
if (!settings.agents?.enabled || !profile) {
  throw new Error(
    `Enable MCP and set up a ${clientName} profile in Doze first.`,
  );
}
const desktop = fileURLToPath(new URL("../", import.meta.url));
const client = new Client({ name: "doze-live-qa", version: "1.0.0" });
const transport = new StdioClientTransport({
  command:
    process.env.DOZE_QA_BINARY ??
    path.join(
      desktop,
      process.platform === "win32"
        ? "src-tauri/target/debug/doze.exe"
        : "src-tauri/target/debug/doze",
    ),
  args: ["--mcp", "--endpoint", path.join(folder, "mcp-endpoint.json")],
  env: { ...process.env, DOZE_MCP_KEY: profile.secret },
  stderr: "inherit",
});
await client.connect(transport);
const call = async (name, args) => {
  const response = await client.callTool({
    name: `doze.${name}`,
    arguments: args,
  });
  if (response.isError) throw new Error(JSON.stringify(response.content));
  return response.structuredContent.result;
};
const session = await call("start_session", {
  reason: `Live Doze QA (${action}): verify wake protection and completion behavior`,
  completion_action: action,
});
console.log(JSON.stringify(session));
console.log("Commands: get, heartbeat, finish, fail, cancel, quit");
const input = createInterface({ input: process.stdin });
try {
  for await (const command of input) {
    const name = {
      get: "get_session",
      heartbeat: "heartbeat",
      finish: "finish_session",
      fail: "fail_session",
      cancel: "cancel_session",
    }[command.trim()];
    if (command.trim() === "quit") break;
    if (!name) continue;
    try {
      console.log(
        JSON.stringify(await call(name, { session_id: session.session_id })),
      );
    } catch (error) {
      console.error(error.message);
    }
  }
} finally {
  input.close();
  process.stdin.pause();
  process.stdin.unref?.();
  await client.close();
}

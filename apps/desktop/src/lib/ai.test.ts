import assert from "node:assert/strict";
import { test } from "node:test";
import { snippet } from "./ai.ts";

test("Claude Code gets the server with one command", () => {
  assert.equal(snippet("claude-code", "emulabzip", false), "claude mcp add emulabzip -- emulabzip mcp");
});

test("a path with spaces is quoted for the shell", () => {
  assert.equal(snippet("claude-code", "/Users/a b/emulabzip", false), 'claude mcp add emulabzip -- "/Users/a b/emulabzip" mcp');
});

test("the changing tools are on only when asked", () => {
  assert.ok(snippet("claude-code", "emulabzip", true).includes("mcp --allow-writes"));
  assert.deepEqual(JSON.parse(snippet("cursor", "emulabzip", false)).mcpServers.emulabzip.args, ["mcp"]);
  assert.deepEqual(JSON.parse(snippet("cursor", "emulabzip", true)).mcpServers.emulabzip.args, ["mcp", "--allow-writes"]);
});

test("each client gets its own file shape", () => {
  assert.equal(JSON.parse(snippet("claude-desktop", "emulabzip", false)).mcpServers.emulabzip.command, "emulabzip");
  const vscode = JSON.parse(snippet("vscode", "emulabzip", false));
  assert.equal(vscode.servers.emulabzip.type, "stdio");
  assert.equal(vscode.mcpServers, undefined);
});

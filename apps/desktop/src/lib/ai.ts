/** Configuration for connecting an AI assistant to EmuLabZip over MCP. */

export type Client = "claude-code" | "claude-desktop" | "cursor" | "vscode";

export const clients: { id: Client; label: string; where: string }[] = [
  { id: "claude-code", label: "Claude Code", where: "Run this in a terminal" },
  { id: "claude-desktop", label: "Claude Desktop", where: "Add to claude_desktop_config.json (Settings, Developer, Edit Config)" },
  { id: "cursor", label: "Cursor", where: "Add to ~/.cursor/mcp.json, or .cursor/mcp.json in a project" },
  { id: "vscode", label: "VS Code", where: "Add to .vscode/mcp.json in a project" },
];

/** Quotes a command for a shell when it has spaces. */
function shell(text: string): string {
  return /[\s"'$`\\]/.test(text) ? `"${text.replace(/(["$`\\])/g, "\\$1")}"` : text;
}

/** The arguments the assistant starts the command with. */
export function mcpArgs(allowWrites: boolean): string[] {
  return allowWrites ? ["mcp", "--allow-writes"] : ["mcp"];
}

export function snippet(client: Client, command: string, allowWrites: boolean): string {
  const args = mcpArgs(allowWrites);
  switch (client) {
    case "claude-code":
      return `claude mcp add emulabzip -- ${[shell(command), ...args].join(" ")}`;
    case "vscode":
      return JSON.stringify({ servers: { emulabzip: { type: "stdio", command, args } } }, null, 2);
    default:
      return JSON.stringify({ mcpServers: { emulabzip: { command, args } } }, null, 2);
  }
}

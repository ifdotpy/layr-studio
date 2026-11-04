// Pass a directory of private, read-only copies of provider JSONL files.
// Name copies codex-*.jsonl or claude-*.jsonl; delete the copies after use.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { sanitize } from "./sanitize.ts";

const directory = process.argv[2];
if (!directory) throw new Error("pass the directory of private log copies");
const output = fileURLToPath(new URL(".", import.meta.url));
function object(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
const found = new Set<string>();
for (const name of readdirSync(directory).sort()) {
  if (!/^(codex|claude)-\d+\.jsonl$/.test(name)) continue;
  for (const line of readFileSync(`${directory}/${name}`, "utf8").split("\n")) {
    let value: unknown;
    try {
      value = JSON.parse(line);
    } catch {
      continue;
    }
    if (!object(value)) continue;
    const payload = object(value.payload) ? value.payload : {};
    const message = object(value.message) ? value.message : {};
    const content = Array.isArray(message.content) ? message.content : [];
    const kind =
      name.startsWith("codex-") &&
      value.type === "event_msg" &&
      payload.type === "item_completed"
        ? "codex-item"
        : name.startsWith("codex-") &&
            value.type === "event_msg" &&
            payload.type === "token_count"
          ? "codex-usage"
          : name.startsWith("codex-") &&
              value.type === "response_item" &&
              payload.type === "custom_tool_call"
            ? "codex-tool"
            : name.startsWith("claude-") &&
                value.type === "assistant" &&
                content.some(
                  (part: unknown) => object(part) && part.type === "tool_use",
                )
              ? "claude-tool"
              : name.startsWith("claude-") && value.type === "cost-state"
                ? "claude-usage"
                : name.startsWith("claude-") &&
                    value.type === "assistant" &&
                    content.some(
                      (part: unknown) => object(part) && part.type === "text",
                    )
                  ? "claude-message"
                  : null;
    if (kind && !found.has(kind)) {
      writeFileSync(`${output}${kind}.json`, sanitize(line));
      found.add(kind);
    }
  }
}
for (const kind of [
  "codex-item",
  "codex-usage",
  "codex-tool",
  "claude-tool",
  "claude-usage",
  "claude-message",
]) {
  if (!found.has(kind))
    throw new Error(`missing ${kind} in the supplied copies`);
}

import { studio_api__agents__models__AgentRole as GeneratedApistudio_api__agents__models__AgentRole } from "../../../generated/api.ts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { flowResult } from "mobx";
import { beforeEach, expect, test, vi } from "vitest";
import {
  McpInventory,
  McpInventoryStore,
  mcpServers,
} from "./McpInventory.tsx";
import type { GetResult } from "../../../api.ts";

const saved = vi.hoisted(() => new Map<string, string>());
vi.mock("../../../browserPersistence.ts", () => ({
  sessionOutboxRequests: () => ({
    key: (scope: string, suffix: string) => scope + suffix,
    get: (scope: string, suffix: string) => saved.get(scope + suffix),
    set: (scope: string, suffix: string, value: string) =>
      saved.set(scope + suffix, value),
    remove: (scope: string, suffix: string) => saved.delete(scope + suffix),
  }),
}));
beforeEach(() => saved.clear());
const inventory: GetResult<"/api/capabilities"> = {
  agent: "agent",
  at: 1,
  model: "model",
  role: GeneratedApistudio_api__agents__models__AgentRole.orchestrator,
  managed: [],
  observed: [],
  observedNative: [],
  nativeInventory: "",
  skills: [],
  servers: [],
  errors: [],
  mcp: [
    {
      name: "files",
      status: "failed",
      error: { message: "Connection refused" },
      config: { secret: "must-not-render" },
    },
    { name: "search", status: "connected" },
  ],
  serversCursor: "next",
  mcpSession: {
    account: "account",
    thread: "thread",
    epoch: 1,
    connection: "connection",
    generation: 1,
  },
};
function render(query = "", session = inventory.mcpSession) {
  const state = new McpInventoryStore(
    "local",
    "agent",
    async () => inventory,
    async () => ({ status: "completed", message: "Reconnect completed." }),
  );
  state.receive({ ...inventory, mcpSession: session });
  return renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(McpInventory, {
        state,
        query,
      }),
    ),
  );
}
test("server rows expose names, status, errors, reconnect and pagination without raw configuration", () => {
  const html = render();
  for (const text of [
    "files",
    "search",
    "failed",
    "connected",
    "Connection refused",
    "Reconnect",
    "Load more servers",
  ])
    expect(html).toContain(text);
  expect(html).not.toContain("must-not-render");
  expect(html).not.toContain("<pre");
});
test("server filters and missing session are explicit", () => {
  expect(render("refused")).toContain("files");
  expect(render("refused")).not.toContain("MCP server search");
  expect(render("missing")).toContain("No matching MCP servers");
  expect(render("", null)).toContain("No active session");
  expect(render("", null)).toContain("disabled");
});
test("malformed entries do not invent actionable server identities", () => {
  expect(
    mcpServers([null, "server", {}, { name: "" }, { serverName: "alias" }]),
  ).toEqual([{ name: "alias", status: "Unknown", error: "" }]);
});

test("a settled reconnect clears only the originating session receipt", async () => {
  let settle!: (result: { status: "failed"; message: string }) => void;
  const response = new Promise<{ status: "failed"; message: string }>(
    (resolve) => {
      settle = resolve;
    },
  );
  const state = new McpInventoryStore(
    "local",
    "agent",
    async () => inventory,
    () => response,
  );
  state.receive(inventory);
  const oldKey = state.requestKey("files");
  const pending = state.reconnectServer("files");
  state.receive({
    ...inventory,
    mcpSession: { ...inventory.mcpSession!, generation: 2 },
  });
  const newKey = state.requestKey("files");
  saved.set(newKey, "new-session-request");
  settle({ status: "failed", message: "The old session ended." });
  await pending;
  expect(saved.get(newKey)).toBe("new-session-request");
  expect(saved.has(oldKey)).toBe(false);
});

test("pagination rejects changed sessions and repeated cursors without replacing rows", async () => {
  const load = vi
    .fn()
    .mockResolvedValueOnce({ ...inventory, mcp: [{ name: "added" }] })
    .mockResolvedValueOnce({ ...inventory, mcpSession: null });
  const state = new McpInventoryStore("local", "agent", load, vi.fn());
  state.receive(inventory);
  await flowResult(state.load("next"));
  expect(state.rows.map((row) => row.name)).toEqual([
    "files",
    "search",
    "added",
  ]);
  await flowResult(state.load("next"));
  expect(state.pageError).toContain("cursor did not advance");
  expect(load).toHaveBeenCalledTimes(1);
  await flowResult(state.load("other"));
  expect(state.pageError).toContain("session changed");
  expect(state.rows).toHaveLength(3);
});
test("unknown reconnect retries the saved identity and disposal rejects late pages", async () => {
  const reconnect = vi
    .fn()
    .mockResolvedValue({ status: "unknown", message: "Unconfirmed" });
  const page = Promise.withResolvers<typeof inventory>();
  const state = new McpInventoryStore(
    "local",
    "agent",
    () => page.promise,
    reconnect,
  );
  state.receive(inventory);
  await flowResult(state.reconnectServer("files"));
  await flowResult(state.reconnectServer("files"));
  expect(reconnect.mock.calls[1]?.[0]).toEqual(reconnect.mock.calls[0]?.[0]);
  expect(saved.get(state.requestKey("files"))).toBe(
    reconnect.mock.calls[0]?.[0].request_id,
  );
  const pending = flowResult(state.load());
  state.dispose();
  page.resolve({ ...inventory, mcp: [] });
  await pending;
  expect(state.rows).toHaveLength(2);
});

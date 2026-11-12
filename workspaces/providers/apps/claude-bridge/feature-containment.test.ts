import { readFile } from "node:fs/promises";
import path from "node:path";
import { expect, test } from "vitest";
import { MAX_UNKNOWN_SDK_DIAGNOSTICS } from "./sdk-message-policy.ts";
import { startBridge, type BridgeHarness } from "./testing/bridge-harness.ts";
import type { BridgeTurnParams } from "./bridge-protocol.ts";
const fixture = new URL("./feature-containment.fixture.mts", import.meta.url);
const params = (
  bridge: BridgeHarness,
  change: Partial<BridgeTurnParams> = {},
) => ({
  threadId: bridge.thread,
  input: [{ type: "text" as const, text: "continue" }],
  ...change,
});
async function successfulTurn(
  bridge: BridgeHarness,
  change: Partial<BridgeTurnParams> = {},
) {
  const { turn } = await bridge.call("turn/start", params(bridge, change));
  const completed = await bridge.note(
    "turn/completed",
    (note) => note.params.turn.id === turn.id,
  );
  expect(completed.params.turn.status).toBe("completed");
}
async function trace(
  bridge: BridgeHarness,
): Promise<Record<string, unknown>[]> {
  return (await readFile(path.join(bridge.root, "containment-trace"), "utf8"))
    .trim()
    .split("\n")
    .map((line) => {
      const value: unknown = JSON.parse(line);
      if (!value || typeof value !== "object" || Array.isArray(value))
        throw Error("Invalid fixture trace");
      return value as Record<string, unknown>;
    });
}
for (const [mode, change, operation] of [
  ["receiver", { model: "claude-sonnet-5-5" }, "setModel"],
  ["restore", { model: "claude-sonnet-5-5" }, "setModel"],
  ["model", { model: "claude-sonnet-5-5" }, "setModel"],
  ["flags", { serviceTier: "priority" }, "applyFlagSettings"],
  ["missing", { model: "claude-sonnet-5-5" }, "setModel"],
  ["missing", { serviceTier: "priority" }, "applyFlagSettings"],
] satisfies [string, Partial<BridgeTurnParams>, string][]) {
  test(`${mode}: ${operation} keeps receiver and admission truthful`, async () => {
    const bridge = await startBridge(fixture, "containment-", {
      env: { FAIL_FEATURE: mode },
    });
    try {
      await successfulTurn(
        bridge,
        mode === "restore" ? { model: "claude-opus-5-5" } : {},
      );
      const refusal = (change: Partial<BridgeTurnParams>) =>
        expect(
          bridge.call("turn/start", params(bridge, change)),
        ).rejects.toMatchObject({
          message: expect.stringContaining("Unsupported Claude " + operation),
          data: { turnStartOutcome: "not_applied" },
        });
      if (mode === "receiver" || mode === "restore") {
        await successfulTurn(bridge, change);
        if (mode === "restore") await refusal({ model: "claude-opus-5-5" });
        else await successfulTurn(bridge, change);
      } else if (mode === "missing") {
        await successfulTurn(bridge);
        await refusal(change);
        await successfulTurn(bridge);
      } else {
        await refusal({});
        await refusal(change);
      }
      const rows = await trace(bridge);
      expect(rows.filter((row) => row.query)).toHaveLength(1);
      if (mode === "restore")
        expect(rows.filter((row) => "input" in row).at(-1)?.actualModel).toBe(
          change.model,
        );
      expect(rows.filter((row) => "input" in row)).toHaveLength(
        mode === "restore" ? 2 : mode === "model" || mode === "flags" ? 1 : 3,
      );
    } finally {
      await bridge.close();
    }
  });
}
test("MCP replacement is refused before removal and leaves the query usable", async () => {
  const bridge = await startBridge(fixture, "containment-");
  try {
    await successfulTurn(bridge);
    await expect(
      bridge.call(
        "turn/start",
        params(bridge, {
          dynamicTools: [
            {
              name: "new_tool",
              description: "new",
              inputSchema: { type: "object" },
            },
          ],
        }),
      ),
    ).rejects.toThrow(/Unsupported Claude live MCP replacement/);
    await successfulTurn(bridge);
    const rows = await trace(bridge);
    expect(rows.some((row) => "servers" in row)).toBe(false);
    expect(rows.filter((row) => row.query)).toHaveLength(1);
  } finally {
    await bridge.close();
  }
});
for (const feature of ["notice", "progress"] as const) {
  test(`malformed optional ${feature} retains the query`, async () => {
    const bridge = await startBridge(fixture, "containment-", {
      env: { FAIL_FEATURE: feature },
    });
    try {
      await successfulTurn(bridge);
      await successfulTurn(bridge);
      expect((await trace(bridge)).filter((row) => row.query)).toHaveLength(1);
    } finally {
      await bridge.close();
    }
  });
}
test("unknown diagnostic retention is bounded across turns", async () => {
  const bridge = await startBridge(fixture, "containment-", {
    env: { FAIL_FEATURE: "diagnostics" },
  });
  let stderr = "";
  bridge.process.stderr?.on("data", (chunk) => (stderr += chunk));
  try {
    await successfulTurn(bridge);
    await successfulTurn(bridge);
    expect(
      stderr
        .split("\n")
        .filter((line) => line.startsWith("Claude SDK unknown message:")),
    ).toHaveLength(MAX_UNKNOWN_SDK_DIAGNOSTICS);
  } finally {
    await bridge.close();
  }
});
test("partial settings failure reapplies requested settings before the next input", async () => {
  const bridge = await startBridge(fixture, "containment-", {
    env: { FAIL_FEATURE: "flags-once" },
  });
  try {
    await successfulTurn(bridge, { model: "claude-opus-5-5" });
    const change = { model: "claude-sonnet-5-5" };
    await expect(
      bridge.call("turn/start", params(bridge, change)),
    ).rejects.toThrow(/Unsupported Claude applyFlagSettings/);
    await successfulTurn(bridge, change);
    const rows = await trace(bridge);
    expect(rows.filter((row) => "model" in row)).toEqual([
      { model: change.model },
      { model: change.model },
    ]);
    expect(rows.filter((row) => "flags" in row)).toHaveLength(2);
    expect(rows.filter((row) => "input" in row)).toHaveLength(2);
    expect(rows.filter((row) => row.query)).toHaveLength(1);
  } finally {
    await bridge.close();
  }
});

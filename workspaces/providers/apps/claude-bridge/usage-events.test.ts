import type { SessionFixtureMessage } from "./fixtures/session-sdk-types.ts";
import { afterEach, test } from "vitest";
import assert from "node:assert/strict";
import { startBridge } from "./testing/bridge-harness.ts";
import { createSessionStore } from "./session-store.ts";
import path from "node:path";

const bridges: Awaited<ReturnType<typeof startBridge>>[] = [];
afterEach(async () => {
  for (const bridge of bridges.splice(0)) await bridge.close();
});
const sdk = new URL("./usage-events.fixture.mts", import.meta.url);
async function run(
  events: readonly SessionFixtureMessage[],
  model = "sonnet",
  env: Record<string, string> = {},
) {
  const bridge = await startBridge(sdk, "claude-usage-", {
    env: { ...env, STUDIO_FIXTURE_EVENTS: JSON.stringify(events) },
  });
  bridges.push(bridge);
  await bridge.call("turn/start", {
    threadId: bridge.thread,
    model,
    input: [{ type: "text", text: "Continue" }],
  });
  const done = await bridge.note((n) => n.method === "turn/completed");
  return { bridge, turn: done.params.turn };
}
const failed = {
  type: "result",
  subtype: "error_during_execution",
  is_error: true,
  errors: ["429 Too Many Requests"],
} satisfies SessionFixtureMessage;

test("a fulfilled occupancy response during the retained query close wait is discarded", async () => {
  const bridge = await startBridge(sdk, "claude-context-close-", {
    clock: true,
    env: {
      STUDIO_FIXTURE_EVENTS: "[]",
      STUDIO_CONTEXT_CLOSE_WAIT: "1",
      STUDIO_DEFER_CONTEXT: "1",
      STUDIO_CLAUDE_STEER_ECHO_GRACE_MS: "100",
      STUDIO_CLAUDE_STALL_EXIT_WAIT_MS: "10000",
    },
  });
  bridges.push(bridge);
  try {
    const start = await bridge.call("turn/start", {
      threadId: bridge.thread,
      input: [{ type: "text", text: "Start" }],
    });
    await bridge.call("turn/steer", {
      threadId: bridge.thread,
      expectedTurnId: start.turn.id,
      clientUserMessageId: "unacknowledged",
      input: [{ type: "text", text: "Continue" }],
    });
    await bridge.note(
      "fixture/signal",
      (note) => note.params.name === "context-pending-1",
    );
    await bridge.advanceTime(100);
    await bridge.note(
      "fixture/signal",
      (note) => note.params.name === "query-closed",
    );
    assert.equal(
      bridge.notes.some((note) => note.method === "turn/completed"),
      false,
    );
    await bridge.writeSignal(".context-1", "600");
    await bridge.note(
      "fixture/signal",
      (note) => note.params.name === "context-settled-1",
    );
    await bridge.call("thread/read", {
      threadId: bridge.thread,
      includeTurns: true,
    });
    assert.equal(
      bridge.notes.some(
        (note) => note.method === "thread/contextUsage/updated",
      ),
      false,
    );
  } finally {
    await bridge.writeSignal(".iterator-exit");
    await bridge.advanceTime(10000);
  }
});

test("a deferred occupancy control cannot block result delivery or turn completion", async () => {
  const { bridge, turn } = await run(
    [{ type: "result", subtype: "success", is_error: false, result: "Done" }],
    "sonnet",
    { STUDIO_DEFER_CONTEXT: "1" },
  );
  assert.equal(turn.status, "completed");
  assert.ok(
    bridge.notes.some((note) => note.method === "thread/tokenUsage/updated"),
  );
  assert.equal(
    bridge.notes.some((note) => note.method === "thread/contextUsage/updated"),
    false,
  );
  await bridge.writeSignal(".context-1", "600");
  const context = await bridge.note(
    (note) => note.method === "thread/contextUsage/updated",
  );
  assert.deepEqual(context.params, {
    threadId: bridge.thread,
    turnId: turn.id,
    contextUsage: { tokens: 600, window: 1000 },
  });
});

test("a deferred occupancy rejection after completion keeps the earlier reading", async () => {
  const { bridge } = await run(
    [{ type: "result", subtype: "success", is_error: false, result: "Done" }],
    "sonnet",
    { STUDIO_DEFER_CONTEXT: "2" },
  );
  await bridge.note((note) => note.method === "thread/contextUsage/updated");
  const started = await bridge.call("turn/start", {
    threadId: bridge.thread,
    input: [{ type: "text", text: "Again" }],
  });
  await bridge.note(
    (note) =>
      note.method === "turn/completed" &&
      note.params.turn.id === started.turn.id,
  );
  await bridge.writeSignal(".context-2", "reject");
  await bridge.note(
    "fixture/signal",
    (note) => note.params.name === "context-settled-2",
  );
  await bridge.call("thread/read", {
    threadId: bridge.thread,
    includeTurns: true,
  });
  assert.equal(
    bridge.notes.filter((note) => note.method === "thread/contextUsage/updated")
      .length,
    1,
  );
});

test("an older deferred occupancy sample cannot overwrite a newer reading", async () => {
  const { bridge } = await run(
    [{ type: "result", subtype: "success", is_error: false, result: "Done" }],
    "sonnet",
    { STUDIO_DEFER_CONTEXT: "1" },
  );
  const started = await bridge.call("turn/start", {
    threadId: bridge.thread,
    input: [{ type: "text", text: "Again" }],
  });
  await bridge.note(
    (note) =>
      note.method === "turn/completed" &&
      note.params.turn.id === started.turn.id,
  );
  await bridge.writeSignal(".context-2", "800");
  await bridge.note(
    "thread/contextUsage/updated",
    (note) => note.params.contextUsage?.tokens === 800,
  );
  await bridge.writeSignal(".context-1", "400");
  await bridge.note(
    "fixture/signal",
    (note) => note.params.name === "context-settled-1",
  );
  await bridge.call("thread/read", {
    threadId: bridge.thread,
    includeTurns: true,
  });
  const readings = bridge.notes.filter(
    (note) => note.method === "thread/contextUsage/updated",
  );
  assert.equal(readings.length, 1);
});

test("result boundary publishes native occupancy separately from billed totals", async () => {
  const { bridge } = await run([
    { type: "result", subtype: "success", is_error: false, result: "Done" },
  ]);
  const usage = bridge.notes.find(
    (note) => note.method === "thread/tokenUsage/updated",
  );
  assert.ok(usage);
  const context = await bridge.note("thread/contextUsage/updated");
  assert.deepEqual(context.params.contextUsage, { tokens: 600, window: 1000 });
  assert.deepEqual(usage.params, {
    threadId: bridge.thread,
    turnId: usage.params.turnId,
    studioUsage: {
      kind: "cumulative",
      scope: "claudeQuery",
      total: {
        uncachedInput: 0,
        cacheReadInput: 0,
        cacheWriteInput: 0,
        output: 0,
      },
      models: [],
      turnId: usage.params.turnId,
    },
  });
});

test("an unavailable occupancy control preserves billing and turn completion", async () => {
  const { bridge, turn } = await run(
    [{ type: "result", subtype: "success", is_error: false, result: "Done" }],
    "sonnet",
    { STUDIO_CONTEXT_USAGE_FAILURE: "1" },
  );
  const usage = bridge.notes.find(
    (note) => note.method === "thread/tokenUsage/updated",
  );
  assert.ok(usage);
  assert.equal("contextUsage" in usage.params, false);
  assert.equal(
    bridge.notes.some((note) => note.method === "thread/contextUsage/updated"),
    false,
  );
  assert.equal(usage.params.studioUsage.kind, "cumulative");
  assert.equal(turn.status, "completed");
});

test("plain 429 without a rate limit event has a typed terminal cause", async () => {
  const { turn } = await run([
    {
      type: "assistant",
      error: "rate_limit",
      // @ts-expect-error This legacy fixture intentionally uses a non-UUID message identity.
      uuid: "error",
      message: {
        id: "error",
        model: "claude-sonnet-4-6",
        content: [{ type: "text", text: "429 Too Many Requests" }],
      },
    },
    failed,
  ]);
  assert.equal(turn.status, "failed");
  assert.equal(turn.error!.codexErrorInfo, "rateLimitExceeded");
});

for (const error of [["overloaded"], "__proto__", "constructor"] as const) {
  test(`unvalidated SDK assistant error ${JSON.stringify(error)} stays unclassified`, async () => {
    const message: SessionFixtureMessage = {
      type: "assistant",
      // @ts-expect-error The SDK forwards parsed JSON without validating its declared error union.
      error,
      message: {
        id: "malformed-error",
        model: "claude-sonnet-4-6",
        content: [{ type: "text", text: "Provider detail" }],
      },
    };
    const original = JSON.stringify(message);
    const { bridge, turn } = await run([
      message,
      { ...failed, errors: ["Provider detail"] },
    ]);
    assert.equal(turn.status, "failed");
    assert.ok(turn.error);
    assert.equal(turn.error.codexErrorInfo, undefined);
    const { session } = await createSessionStore(
      path.join(bridge.root, "state"),
    ).readAll(bridge.thread);
    const saved = session.turns.find((entry) => entry.id === turn.id);
    assert.ok(saved);
    assert.equal(saved.apiErrorInfo, null);
    assert.equal(JSON.stringify(message), original);
  });
}

test("terminal result with only a plain HTTP 429 has a typed cause", async () => {
  const { turn } = await run([failed]);
  assert.equal(turn.error!.codexErrorInfo, "rateLimitExceeded");
});

test("retry and fallback details survive as readable thread items", async () => {
  const { bridge } = await run([
    {
      type: "system",
      subtype: "api_retry",
      // @ts-expect-error This legacy fixture intentionally uses a non-UUID message identity.
      uuid: "retry-1",
      attempt: 2,
      max_retries: 5,
      retry_delay_ms: 1500,
      error_status: 429,
      error: "rate_limit",
    },
    {
      type: "system",
      subtype: "model_refusal_fallback",
      trigger: "refusal",
      direction: "retry",
      request_id: null,
      // @ts-expect-error This legacy fixture intentionally uses a non-UUID message identity.
      uuid: "fallback-1",
      original_model: "claude-opus-4-6",
      fallback_model: "claude-sonnet-4-6",
      content:
        "Changed from claude-opus-4-6 to claude-sonnet-4-6 after refusal.",
    },
    {
      type: "system",
      subtype: "model_refusal_fallback",
      trigger: "refusal",
      direction: "sticky",
      request_id: null,
      // @ts-expect-error This legacy fixture intentionally uses a non-UUID message identity.
      uuid: "consent-1",
      original_model: "claude-opus-4-6",
      fallback_model: "claude-sonnet-4-6",
      content:
        "Changed from claude-opus-4-6 to claude-sonnet-4-6 for this session.",
    },
    { type: "result", subtype: "success", is_error: false, result: "Done" },
  ]);
  const read = await bridge.call("thread/read", {
    threadId: bridge.thread,
    includeTurns: true,
  });
  const items = read.thread.turns.flatMap((turn) => turn.items);
  const retry = items.find((item) => item.id === "retry-1");
  assert.ok(retry, "retry item persisted");
  assert.match(retry.text!, /2.*5/);
  assert.match(retry.text!, /1500 ms/);
  assert.match(retry.text!, /429/);
  for (const id of ["fallback-1", "consent-1"]) {
    const item = items.find((item) => item.id === id);
    assert.ok(item, id + " persisted");
    assert.match(item.text!, /claude-opus-4-6/);
    assert.match(item.text!, /claude-sonnet-4-6/);
    assert.ok(
      bridge.notes.some(
        (n) => n.method === "item/completed" && n.params.item.id === id,
      ),
    );
  }
});

test("an Opus rejection does not attach its reset to a Sonnet failure", async () => {
  const { turn } = await run([
    {
      type: "rate_limit_event",
      rate_limit_info: {
        rateLimitType: "seven_day_opus",
        status: "rejected",
        utilization: 1,
        resetsAt: 9000,
      },
    },
    {
      type: "result",
      subtype: "error_during_execution",
      is_error: true,
      errors: ["Unrelated failure"],
    },
  ]);
  assert.equal(turn.error!.codexErrorInfo, undefined);
  assert.equal(turn.error!.resetsAt, undefined);
});

for (const delta of [
  { type: "text_delta", text: "text" },
  { type: "thinking_delta", thinking: "reasoning" },
  { type: "input_json_delta", partial_json: "{" },
] as const) {
  test(`first model output records ${delta.type} once per turn`, async () => {
    const { bridge, turn } = await run([
      {
        type: "stream_event",
        event: { type: "content_block_delta", index: 0, delta },
      },
      {
        type: "stream_event",
        event: { type: "content_block_delta", index: 0, delta },
      },
      failed,
    ]);
    const first = bridge.notes.filter(
      (note) => note.method === "item/modelOutput/firstDelta",
    );
    assert.equal(first.length, 1);
    assert.deepEqual(first[0]!.params, {
      threadId: bridge.thread,
      turnId: turn.id,
    });
  });
}

test("empty model output deltas do not start first-output timing", async () => {
  const { bridge } = await run([
    {
      type: "stream_event",
      event: {
        type: "content_block_delta",
        index: 0,
        delta: { type: "text_delta", text: "" },
      },
    },
    {
      type: "stream_event",
      event: {
        type: "content_block_delta",
        index: 0,
        delta: { type: "thinking_delta", thinking: "" },
      },
    },
    {
      type: "stream_event",
      event: {
        type: "content_block_delta",
        index: 0,
        delta: { type: "input_json_delta", partial_json: "" },
      },
    },
    failed,
  ]);
  assert.equal(
    bridge.notes.filter((note) => note.method === "item/modelOutput/firstDelta")
      .length,
    0,
  );
});

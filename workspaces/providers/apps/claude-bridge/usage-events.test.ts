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
async function run(events: readonly SessionFixtureMessage[], model = "sonnet") {
  const bridge = await startBridge(sdk, "claude-usage-", {
    env: { STUDIO_FIXTURE_EVENTS: JSON.stringify(events) },
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

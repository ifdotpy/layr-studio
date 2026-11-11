import assert from "node:assert/strict";
import { test } from "vitest";
import {
  decodeBridgeInput,
  decodeBridgeResult,
  decodeBridgeServerResult,
} from "./bridge-decode.ts";

test("elicitation accepts the nullable decline and cancel replies Rust sends", () => {
  for (const action of ["decline", "cancel"] as const) {
    const reply = { action, content: null };
    assert.deepEqual(
      decodeBridgeServerResult("mcpServer/elicitation/request", reply),
      reply,
    );
  }
});

test("permission-bearing requests reject unknown Studio wire values", () => {
  for (const method of [
    "thread/start",
    "thread/resume",
    "thread/fork",
    "turn/start",
  ]) {
    for (const invalid of [
      { approvalPolicy: "future-policy" },
      ...(method === "turn/start" ? [] : [{ sandbox: "future-sandbox" }]),
    ]) {
      assert.throws(() =>
        decodeBridgeInput(
          JSON.stringify({
            id: 1,
            method,
            params: { threadId: "thread", input: [], ...invalid },
          }),
        ),
      );
    }
  }
});

test("the generated request union validates every consumed known field", () => {
  for (const params of [null, [], "text", 1])
    assert.throws(() =>
      decodeBridgeInput(
        JSON.stringify({ id: 1, method: "initialize", params }),
      ),
    );
  for (const params of [
    {},
    { threadId: 1, input: [] },
    { threadId: "thread", input: [{}] },
    { threadId: "thread", input: [{ type: "text", text: 4 }] },
    { threadId: "thread", input: [], model: 7 },
  ])
    assert.throws(() =>
      decodeBridgeInput(
        JSON.stringify({ id: "start", method: "turn/start", params }),
      ),
    );
  assert.deepEqual(
    decodeBridgeInput(
      JSON.stringify({
        id: 1,
        method: "turn/start",
        params: {
          threadId: "thread",
          input: [{ type: "text", text: "hello", inputOrigin: "human" }],
        },
      }),
    ),
    {
      id: 1,
      method: "turn/start",
      params: {
        threadId: "thread",
        input: [{ type: "text", text: "hello", inputOrigin: "human" }],
      },
    },
  );
});

test("replies retain the pending request's method contract", () => {
  assert.deepEqual(
    decodeBridgeServerResult("item/commandExecution/requestApproval", {
      decision: "decline",
    }),
    { decision: "decline" },
  );
  for (const value of [
    null,
    {},
    { decision: "invented" },
    { decision: false },
    { action: "accept" },
  ])
    assert.throws(() =>
      decodeBridgeServerResult("item/commandExecution/requestApproval", value),
    );
  assert.throws(() =>
    decodeBridgeServerResult("mcpServer/elicitation/request", {
      action: "invented",
    }),
  );
  assert.throws(() => decodeBridgeResult("turn/steer", { turnId: 4 }));
});

test("ambiguous and malformed response envelopes never reach a waiter", () => {
  for (const value of [
    { id: {}, result: {} },
    { id: 1 },
    { id: 1, result: {}, error: { code: 1, message: "ambiguous" } },
    { id: 1, error: { message: "missing code" } },
    { id: 1, method: "initialize", params: {}, result: {} },
  ])
    assert.throws(() => decodeBridgeInput(JSON.stringify(value)));
});

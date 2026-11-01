import { WorkStatus as GeneratedApiWorkStatus } from "../../generated/api.ts";
import { expect, test } from "vitest";
import type { PostBody } from "../../api";
import {
  DEFAULT_GATE_TIMEOUT_MS,
  MAX_GATE_TIMEOUT_MS,
  MAX_RESULT_SCHEMA_BYTES,
} from "../../generated/taskReview";
import {
  gateFields,
  parseGate,
  parseResultSchema,
  requiresGateOverride,
  taskStatus,
  type ReviewTask,
} from "./taskReviewModel";

const task = {
  id: "task",
  title: "Task",
  rootId: "lead",
  created: 1,
  updated: 1,
  version: 1,
  status: GeneratedApiWorkStatus.review,
  policy: {
    blockedBy: [],
    displayStatus: GeneratedApiWorkStatus.review,
    acceptAvailable: true,
    gateOverrideRequired: false,
  },
  gate: { command: "check", timeout_ms: DEFAULT_GATE_TIMEOUT_MS },
} satisfies ReviewTask;
test("generated task commands require their action payloads", () => {
  const create = {
    action: "create",
    title: "Task",
  } satisfies PostBody<"/api/work">;
  const submit = {
    action: "submit",
    task_id: "task",
    result: { answer: null },
    checks: "checked",
    revision: "abc",
  } satisfies PostBody<"/api/work">;
  // @ts-expect-error A task cancellation requires its reason.
  const incomplete: PostBody<"/api/work"> = {
    action: "cancel",
    task_id: "task",
  };
  expect(create.action).toBe("create");
  expect(submit.result).toEqual({ answer: null });
  expect(incomplete.action).toBe("cancel");
});

test("gate input validates the Rust-owned timeout bounds and supports disable", () => {
  expect(parseGate("   ", "bad")).toBeNull();
  expect(parseGate("check", "1")).toEqual({ command: "check", timeout_ms: 1 });
  expect(parseGate("check", String(MAX_GATE_TIMEOUT_MS))).toEqual({
    command: "check",
    timeout_ms: MAX_GATE_TIMEOUT_MS,
  });
  for (const timeout of ["0", "1.5", "bad", String(MAX_GATE_TIMEOUT_MS + 1)])
    expect(() => parseGate("check", timeout)).toThrow("timeout");
  expect(gateFields("check").timeout).toBe(String(DEFAULT_GATE_TIMEOUT_MS));
});

test("the schema editor preserves boolean schemas and rejects invalid JSON, shapes and size", () => {
  expect(parseResultSchema(" ")).toBeUndefined();
  expect(parseResultSchema("false")).toBe(false);
  expect(parseResultSchema('{"type":"object"}')).toEqual({ type: "object" });
  for (const text of ["{", "[]", "null", '"string"', "3"])
    expect(() => parseResultSchema(text)).toThrow();
  expect(() =>
    parseResultSchema(
      JSON.stringify({ description: "x".repeat(MAX_RESULT_SCHEMA_BYTES) }),
    ),
  ).toThrow("bytes");
});

test("the list uses dependency blocking in preference to the persisted ready status", () => {
  expect(
    taskStatus({
      ...task,
      status: GeneratedApiWorkStatus.ready,
      policy: { ...task.policy, displayStatus: GeneratedApiWorkStatus.blocked },
    }),
  ).toBe("blocked");
});

test("uses the server's gate override decision", () => {
  const serverTask = {
    ...task,
    policy: { ...task.policy, gateOverrideRequired: false },
  };
  expect(requiresGateOverride(serverTask)).toBe(false);
  expect(
    requiresGateOverride({
      ...serverTask,
      policy: { ...serverTask.policy, gateOverrideRequired: true },
    }),
  ).toBe(true);
});

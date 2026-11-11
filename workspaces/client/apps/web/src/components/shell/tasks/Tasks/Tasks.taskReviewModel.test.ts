import { expect, test } from "vitest";
import {
  DEFAULT_GATE_TIMEOUT_MS,
  MAX_GATE_TIMEOUT_MS,
  MAX_RESULT_SCHEMA_BYTES,
} from "../../../../generated/taskReview.ts";
import {
  gateFields,
  parseGate,
  parseResultSchema,
  taskDraft,
  taskBody,
} from "./taskReviewModel.ts";

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

test("preserves stored dependency order for unchanged selection and serializes changed selections", () => {
  const draft = taskDraft();
  draft.dependencies = ["B", "A"];
  const values = new FormData();
  values.append("dependencies", "A");
  values.append("dependencies", "B");
  expect(taskBody(draft, undefined, values).dependencies).toEqual(["B", "A"]);
  values.delete("dependencies");
  values.append("dependencies", "A");
  expect(taskBody(draft, undefined, values).dependencies).toEqual(["A"]);
  values.append("dependencies", "C");
  expect(taskBody(draft, undefined, values).dependencies).toEqual(["A", "C"]);
  values.delete("dependencies");
  expect(taskBody(draft, undefined, values).dependencies).toEqual([]);
});

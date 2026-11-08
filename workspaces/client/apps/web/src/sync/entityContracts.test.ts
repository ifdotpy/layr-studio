import { expect, it } from "vitest";
import { parseEntityPayload } from "./entityContracts.ts";

it.each([
  null,
  [],
  1,
  "{}",
  {},
  { collection: "future", id: "x", value: {} },
  { collection: "agent", id: "", value: {} },
  { collection: "agent", id: "a", value: null },
])("rejects malformed entity envelope %j", (value) => {
  expect(parseEntityPayload(value)).toBeNull();
});
it("preserves native DTO fields and identity after validating the cache envelope", () => {
  const row = {
    collection: "agent",
    id: "a",
    value: { id: "a", name: 2, future: "kept" },
  };
  expect(parseEntityPayload(row)).toBe(row);
});

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { sanitize } from "./sanitize.ts";

test("distinct object keys remain distinct while repeated keys stay stable", () => {
  const source = '{"b":1,"d":2,"nested":{"b":3,"d":4},"b":5}';
  const output = sanitize(source);
  const keys = [...output.matchAll(/"([^"]+)":/g)].map((match) => match[1]);
  assert.notEqual(keys[0], keys[1]);
  assert.notEqual(keys[3], keys[4]);
  assert.equal(keys[0], keys[5]);
});

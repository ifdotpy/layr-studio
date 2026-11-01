import { LimitColor } from "../generated/api";
import type { components } from "../generated/api";
import { expect, it } from "vitest";
import { accountBuckets } from "./accountBuckets";

it("selects the server's unknown aggregate at the first expiry", () => {
  const current: components["schemas"]["BucketState"] = {
    buckets: [],
    remaining: 20,
    color: LimitColor.answer,
    low: false,
  };
  const next: components["schemas"]["BucketState"] = {
    buckets: [],
    remaining: null,
    color: LimitColor.paused,
    low: false,
  };
  const projection = { current, next, validUntil: 110 };
  expect(accountBuckets(projection, 109)).toBe(current);
  expect(accountBuckets(projection, 110)).toBe(next);
});

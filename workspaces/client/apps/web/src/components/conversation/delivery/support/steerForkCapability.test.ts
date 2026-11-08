import assert from "node:assert/strict";
import { test } from "vitest";
import { providerSupports } from "../../../../generated/providerKind.ts";

// The conversation shows "Send now" and "Branch after this turn" only for these capabilities.
test("steer and fork follow the provider capability list", () => {
  for (const provider of ["claude", "codex"]) {
    assert.equal(providerSupports(provider, "steer"), true);
    assert.equal(providerSupports(provider, "fork"), true);
  }
  assert.equal(providerSupports("future", "steer"), false);
  assert.equal(providerSupports("future", "fork"), false);
});

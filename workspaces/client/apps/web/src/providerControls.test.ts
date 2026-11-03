import assert from "node:assert/strict";
import { test } from "vitest";
import { providerCapabilities } from "./generated/providerKind.ts";
import { capabilityControls } from "./providerControls.ts";

test("steer and fork show only when the provider lists them", () => {
  assert.deepEqual(capabilityControls(["steer", "fork"]), {
    steer: true,
    fork: true,
  });
  assert.deepEqual(capabilityControls(["rewind", "compaction"]), {
    steer: false,
    fork: false,
  });
  assert.deepEqual(capabilityControls([]), { steer: false, fork: false });
});

test("a provider with the steer capability but no fork shows only steer", () => {
  assert.deepEqual(capabilityControls(["steer"]), { steer: true, fork: false });
  assert.deepEqual(capabilityControls(["fork"]), { steer: false, fork: true });
});

test("the generated lists of both providers show steer and fork", () => {
  for (const provider of ["claude", "codex"]) {
    assert.deepEqual(capabilityControls(providerCapabilities(provider)), {
      steer: true,
      fork: true,
    });
  }
  assert.deepEqual(capabilityControls(providerCapabilities("future")), {
    steer: false,
    fork: false,
  });
});

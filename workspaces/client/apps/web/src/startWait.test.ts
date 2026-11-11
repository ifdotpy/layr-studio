import { expect, test } from "vitest";
import { AgentStatus, StartWaitReason } from "./generated/api.ts";
import { startWaitLabel } from "./startWait.ts";
import type { Agent } from "./types.ts";
const queued = {
  status: AgentStatus.queued,
  epoch: 3,
  startWait: {
    reason: StartWaitReason.model_catalog,
    epoch: 3,
  },
} satisfies Pick<Agent, "status" | "epoch" | "startWait">;

test("a wait of another epoch or a started turn shows nothing", () => {
  expect(startWaitLabel({ ...queued, epoch: 4 })).toBeNull();
  expect(startWaitLabel({ ...queued, status: AgentStatus.running })).toBeNull();
  expect(startWaitLabel({ ...queued, startWait: null })).toBeNull();
});

test("the owner reason remains visible without invented account gate details", () => {
  for (const [reason, label] of [
    [StartWaitReason.disk, "Waiting for free disk space"],
    [StartWaitReason.model_catalog, "Waiting for the model catalog"],
    [StartWaitReason.provider_session, "Waiting for the provider session"],
    [StartWaitReason.dependency, "Waiting for a preparation dependency"],
  ] as const) {
    expect(startWaitLabel({ ...queued, startWait: { reason, epoch: 3 } })).toBe(
      label,
    );
  }
});

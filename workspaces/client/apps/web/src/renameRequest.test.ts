import { autorun } from "mobx";
import { transientTestFixture } from "./sync/subscriptions/transientTestFixture.ts";
import { agentId, requestId } from "./generated/observableResources.ts";
import { resourceData } from "./store/runtime.ts";
import { afterEach, expect, it, vi } from "vitest";
import { submitRename } from "./renameRequest.ts";
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
it("a pending rename sends one POST and waits for the entity stream", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("window", globalThis);
  const post = vi.fn(async () => ({ id: "agent", status: "pending" as const }));
  const recover = vi.fn(async () => {});
  await submitRename({ id: "agent", request_id: "request" }, post);
  await vi.advanceTimersByTimeAsync(90_000);
  expect(post).toHaveBeenCalledTimes(1);
  expect(recover).not.toHaveBeenCalled();
});
it("a lost response preserves uncertainty without disturbing the entity stream", async () => {
  const lost = new TypeError("response lost");
  const post = vi.fn(async () => {
    throw lost;
  });
  const recover = vi.fn(async () => {});
  await expect(
    submitRename({ id: "agent", request_id: "request" }, post),
  ).rejects.toBe(lost);
  expect(post).toHaveBeenCalledTimes(1);
  expect(recover).not.toHaveBeenCalled();
});

it("delivers request-scoped rename failure through the generated facade without HTTP", async () => {
  vi.useFakeTimers();
  const f = transientTestFixture();
  const slot = f.store.renameProgress(agentId("agent"), requestId("request"));
  const stop = autorun(() => {
    void slot.current;
  });
  try {
    await vi.advanceTimersByTimeAsync(100);
    const ref = {
      kind: "rename-progress",
      agentId: "agent",
      requestId: "request",
    } as const;
    const value = {
      id: "agent",
      request_id: "request",
      status: "failed",
      error: "No title",
    } as const;
    f.emit(ref, {
      epoch: "one",
      revision: 1,
      progress: { resource: ref, value },
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(resourceData(slot.current)).toEqual(value);
  } finally {
    stop();
    f.close();
  }
});

import { createApiStorage } from "../../../apiStorage.ts";
import { OBSERVABLE_INTEREST_DELAY_MS } from "../../../store/interest.ts";
import { autorun, flowResult } from "mobx";
import { now } from "../../../clock.ts";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ApiError } from "../../../api.ts";
import { transientTestFixture } from "../../../sync/subscriptions/transientTestFixture.ts";
import { accountsFixture } from "../Accounts/Accounts.fixture.ts";
import { ClaudeAuthFlow } from "./ClaudeAuthFlow.ts";
let owner: ReturnType<typeof accountsFixture>,
  stream: ReturnType<typeof transientTestFixture>;
const flows: ClaudeAuthFlow[] = [];
function flow(accountKey?: string) {
  const ready = vi.fn();
  const current = new ClaudeAuthFlow(
    owner.areas.scope.services.api,
    stream.store,
    { ...stream.subscriptions, retryNow: vi.fn() },
    {
      scope: "fixture",
      ...(accountKey ? { accountKey } : {}),
      label: "Profile",
      email: "person@example.test",
      onReady: ready,
    },
  );
  flows.push(current);
  return { current, ready };
}
function receipt(
  current: ClaudeAuthFlow,
  status: "pending" | "ready" = "pending",
) {
  return {
    requestId: current.requestId,
    accountKey: "personal",
    status,
    verificationUrl: "https://claude.com/cai/oauth/authorize",
  } as const;
}
beforeEach(() => {
  vi.useFakeTimers();
  owner = accountsFixture();
  stream = transientTestFixture();
});
afterEach(() => {
  for (const flow of flows.splice(0)) flow.dispose();
  stream.close();
  owner.close();
  vi.useRealTimers();
});

it("preserves the saved attempt when starting a replacement is refused during sync", async () => {
  const { current } = flow();
  current.persist({
    requestId: "previous",
    startRequestId: "previous-start",
    label: "Profile",
    email: "person@example.test",
    createdAt: Date.now(),
  });
  const previous = current.flow;
  const api = owner.areas.scope.services.api;
  api.setMutationReadiness(() => false);
  owner.post.mockImplementationOnce(async () => {
    if (!api.mutationReady()) throw new Error("Studio is syncing");
    return receipt(current);
  });
  await flowResult(current.start("replacement@example.test", true));
  expect(current.flow).toBe(previous);
  expect(api.saved(current.storageKey, "")).toBe("previous");
  expect(flow().current.flow).toEqual(previous);
  expect(owner.post).not.toHaveBeenCalled();
  expect(current.mutationReady).toBe(false);
});

it("locks start synchronously and retries the identical saved add payload after uncertainty and reload", async () => {
  const { current } = flow();
  let reject!: (reason: Error) => void;
  owner.post.mockImplementationOnce(
    () =>
      new Promise((_resolve, failure) => {
        reject = failure;
      }),
  );
  const first = flowResult(current.start());
  await flowResult(current.start("other@example.test", true));
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(owner.post).toHaveBeenCalledOnce();
  reject(new Error("Lost reply"));
  await first;
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(current.error).toBe("Lost reply");
  current.dispose();
  const restored = flow().current;
  owner.post.mockResolvedValueOnce(receipt(restored));
  await flowResult(restored.start("changed@example.test"));
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(owner.post.mock.calls[1]).toEqual(owner.post.mock.calls[0]);
});
it("retains an uncertain authorization submission and never resends it or replaces its payload", async () => {
  const { current } = flow();
  owner.post.mockImplementationOnce(async () => receipt(current));
  await flowResult(current.start());
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  current.setCode("CODE#STATE");
  owner.post.mockRejectedValueOnce(new Error("Lost reply"));
  await flowResult(current.run("code"));
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  const original = owner.post.mock.calls[1];
  expect(original?.[1]).toMatchObject({ code: "CODE#STATE" });
  const id = current.flow?.codeRequestId;
  expect(id).toBe(original?.[2].requestId);
  current.setCode("DIFFERENT#STATE");
  await flowResult(current.run("code"));
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(owner.post).toHaveBeenCalledTimes(2);
  expect(current.codeSubmissionUncertain).toBe(true);
  current.dispose();
  const restored = flow().current;
  expect(restored.flow?.codeRequestId).toBe(id);
  expect(restored.codeSubmissionUncertain).toBe(true);
  await flowResult(restored.run("code"));
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(owner.post).toHaveBeenCalledTimes(2);
});
it("assigns a new mutation identity after a definitive code rejection and a corrected payload", async () => {
  const { current } = flow();
  owner.post.mockImplementationOnce(async () => receipt(current));
  await flowResult(current.start());
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  current.setCode("123456");
  owner.post.mockRejectedValueOnce(
    new ApiError("Paste code#state", 400, { outcome: "not_applied" }),
  );
  await flowResult(current.run("code"));
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(current.code).toBe("123456");
  expect(current.canSubmitCode).toBe(true);
  current.setCode("CODE#STATE");
  owner.post.mockImplementationOnce(async () => ({
    ...receipt(current),
    codeSubmitted: true,
  }));
  await flowResult(current.run("code"));
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(owner.post.mock.calls[2]?.[2].requestId).not.toBe(
    owner.post.mock.calls[1]?.[2].requestId,
  );
  expect(owner.post.mock.calls[2]?.[1]).toMatchObject({
    login_id: current.requestId,
    code: "CODE#STATE",
  });
  expect(current.verificationPending).toBe(true);
  expect(current.canSubmitCode).toBe(false);
});
it("keeps a cancel request identity across uncertain retries", async () => {
  const { current } = flow();
  owner.post.mockImplementationOnce(async () => receipt(current));
  await flowResult(current.start());
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  owner.post.mockRejectedValueOnce(new Error("Lost cancel"));
  await flowResult(current.run("cancel"));
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  owner.post.mockResolvedValueOnce({
    ...receipt(current),
    status: "cancelled",
  });
  await flowResult(current.run("cancel"));
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(owner.post.mock.calls[2]).toEqual(owner.post.mock.calls[1]);
  expect(current.terminal).toBe(true);
});
it("rejects a receipt for another request without publishing it", async () => {
  const { current } = flow();
  owner.post.mockResolvedValueOnce({
    requestId: "other",
    accountKey: "personal",
    status: "ready",
  });
  await flowResult(current.start());
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(current.error).toContain("another account or request");
  expect(current.receipt).toBeNull();
});
it("publishes completion once and clears its saved add attempt before a reload", async () => {
  const { current, ready } = flow();
  owner.post.mockImplementationOnce(async () => receipt(current, "ready"));
  await flowResult(current.start());
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(ready).toHaveBeenCalledOnce();
  stream.subscriptions.publish(
    { kind: "auth-progress", requestId: current.requestId },
    receipt(current, "ready"),
  );
  expect(ready).toHaveBeenCalledOnce();
  current.dispose();
  expect(flow().current.requestId).toBe("");
});
it("disposes receipt reactions before delayed completion", async () => {
  const { current, ready } = flow();
  owner.post.mockRejectedValueOnce(new Error("Lost reply"));
  await flowResult(current.start());
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  current.dispose();
  stream.subscriptions.publish(
    { kind: "auth-progress", requestId: current.requestId },
    receipt(current, "ready"),
  );
  await vi.advanceTimersByTimeAsync(6000);
  expect(ready).not.toHaveBeenCalled();
});

it("keeps saved-account cancellation identity after an uncertain response and dialog remount", async () => {
  owner.areas.scope.services.api.stableRequestId = () => "saved-start-mutation";
  owner.areas.scope.services.api.clearStableRequestId = vi.fn();
  const { current } = flow("personal");
  owner.post.mockImplementationOnce(async () => receipt(current));
  await flowResult(current.start());
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  owner.post.mockRejectedValueOnce(new Error("Lost cancel reply"));
  await flowResult(current.run("cancel"));
  const cancellation = owner.post.mock.calls[1];
  expect(current.error).toBe("Lost cancel reply");
  current.dispose();
  const restored = flow("personal").current;
  expect(restored.requestId).toBe(current.requestId);
  owner.post.mockResolvedValueOnce({
    ...receipt(restored),
    status: "cancelled",
  });
  await flowResult(restored.run("cancel"));
  await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
  expect(owner.post.mock.calls[2]).toEqual(cancellation);
  expect(restored.terminal).toBe(true);
});

it("keeps the Codex cancellation request identity after a lost reply and dialog remount", async () => {
  const { CodexAuthFlow } = await import("./CodexAuthFlow.ts");
  const services = {
    api: owner.areas.scope.services.api,
    store: {
      ...owner.areas.scope.store,
      authProgress: stream.store.authProgress,
    },
    resources: {
      ...owner.areas.scope.services.resources,
      ...stream.subscriptions,
    },
  };
  Object.assign(
    services.api,
    createApiStorage(owner.areas.scope.services.storage),
  );
  const options = {
    scope: "fixture-codex",
    accountKey: "personal",
    email: "",
    label: "Personal",
  };
  const current = new CodexAuthFlow(services, options);
  let restored: InstanceType<typeof CodexAuthFlow> | undefined;
  try {
    owner.post.mockImplementationOnce(
      async (
        _path: unknown,
        body: import("../../../api.ts").PostBody<"/api/accounts/login">,
      ) => ({
        requestId: body.login_id,
        accountKey: "personal",
        status: "pending",
      }),
    );
    await flowResult(current.run("start"));
    await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
    owner.post.mockRejectedValueOnce(new Error("Lost cancel reply"));
    await flowResult(current.run("cancel"));
    const cancellation = owner.post.mock.calls[1];
    expect(current.error).toBe("Lost cancel reply");
    current.dispose();
    restored = new CodexAuthFlow(services, options);
    expect(restored.requestId).toBe(current.requestId);
    owner.post.mockResolvedValueOnce({
      requestId: current.requestId,
      accountKey: "personal",
      status: "cancelled",
    });
    await flowResult(restored.run("cancel"));
    await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
    expect(owner.post.mock.calls[2]).toEqual(cancellation);
    expect(restored.receipt?.status).toBe("cancelled");
  } finally {
    current.dispose();
    restored?.dispose();
  }
});

it("offers a new attempt at the exact unknown-request deadline regardless of clock tick phase", async () => {
  const stopClock = autorun(() => now());
  await vi.advanceTimersByTimeAsync(100);
  const { current } = flow();
  current.persist({
    requestId: "unknown",
    startRequestId: "",
    label: "Profile",
    email: "",
    createdAt: Date.now(),
    unknownSince: Date.now(),
  });
  let expired = false;
  const stop = autorun(() => {
    expired = current.unknownTooLong;
  });
  try {
    await vi.advanceTimersByTimeAsync(4999);
    expect(expired).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect(expired).toBe(true);
  } finally {
    stop();
    stopClock();
  }
});

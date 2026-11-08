import { autorun } from "mobx";
import { expect, it, vi } from "vitest";
import { requestId } from "../../generated/observableResources.ts";
import { resourceData } from "../../store/runtime.ts";
import { transientTestFixture } from "./transientTestFixture.ts";
import type { components } from "../../generated/api.ts";

it("auth progress opens no HTTP read and only a stream-announced missing receipt recovers", async () => {
  vi.useFakeTimers();
  type Receipt = components["schemas"]["ClaudeLoginResponse"];
  let finish!: (value: Receipt) => void;
  const recover = vi.fn(
    () =>
      new Promise<Receipt>((resolve) => {
        finish = resolve;
      }),
  );
  const f = transientTestFixture({ authProgress: recover, voice: vi.fn() });
  const ref = { kind: "auth-progress", requestId: "login" } as const;
  const slot = f.store.authProgress(requestId("login"));
  const stop = autorun(() => {
    void slot.current;
  });
  try {
    await vi.advanceTimersByTimeAsync(100);
    expect(recover).not.toHaveBeenCalled();
    f.emit(ref, { epoch: "one", revision: 1 });
    await vi.advanceTimersByTimeAsync(0);
    expect(recover).not.toHaveBeenCalled();
    f.emit(ref, {
      epoch: "one",
      revision: 2,
      progress: { resource: ref, value: null },
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(recover).toHaveBeenCalledExactlyOnceWith("login");
    const ready: Receipt = {
      requestId: "login",
      accountKey: "account",
      status: "ready",
    };
    f.emit(ref, {
      epoch: "one",
      revision: 3,
      progress: { resource: ref, value: ready },
    });
    finish({ ...ready, status: "pending" });
    await vi.advanceTimersByTimeAsync(0);
    expect(resourceData(slot.current)?.status).toBe("ready");
    expect(recover).toHaveBeenCalledOnce();
  } finally {
    stop();
    f.close();
    vi.useRealTimers();
  }
});

it("a late pending command reply cannot downgrade a terminal auth receipt", async () => {
  vi.useFakeTimers();
  const f = transientTestFixture();
  const ref = { kind: "auth-progress", requestId: "login" } as const;
  const slot = f.store.authProgress(requestId("login"));
  const stop = autorun(() => {
    void slot.current;
  });
  try {
    await vi.advanceTimersByTimeAsync(100);
    f.emit(ref, {
      epoch: "one",
      revision: 1,
      progress: {
        resource: ref,
        value: { requestId: "login", accountKey: "account", status: "ready" },
      },
    });
    await vi.advanceTimersByTimeAsync(0);
    f.subscriptions.publish(ref, {
      requestId: "login",
      accountKey: "account",
      status: "pending",
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(resourceData(slot.current)?.status).toBe("ready");
  } finally {
    stop();
    f.close();
    vi.useRealTimers();
  }
});

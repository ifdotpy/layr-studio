import { autorun, makeObservable, observable } from "mobx";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ResourceVersion } from "../sync/resourceEvents.ts";
import { createSubscriptions } from "../sync/subscriptions/module.ts";
import { subscriptionKey } from "../sync/subscriptions/contracts.ts";
import type { ResourceRef } from "../sync/subscriptions/contracts.ts";
import { createObservableResourceRuntime } from "./runtime.ts";

const ref = { kind: "native-layr-settings" } as const;
class SettingsModel {
  storeRoot: string | null;
  constructor(storeRoot: string | null) {
    this.storeRoot = storeRoot;
    makeObservable(this, { storeRoot: observable });
  }
}
function fixture() {
  const callbacks = new Map<string, (version?: ResourceVersion) => void>();
  const removed = vi.fn();
  const added = vi.fn(
    (resource: ResourceRef, accept: (v?: ResourceVersion) => void) => {
      callbacks.set(subscriptionKey(resource), accept);
      return removed;
    },
  );
  const subscriptions = createSubscriptions({
    watchResourceChanges: added,
    watchResourceConnection: (accept) => {
      accept("live", "open", {});
      return () => {};
    },
  });
  const runtime = createObservableResourceRuntime(subscriptions);
  const slot = runtime.slot({
    key: subscriptionKey(ref),
    ref,
    create: (dto) => new SettingsModel(dto.storeRoot),
    apply: (model, dto) => {
      if (model.storeRoot !== dto.storeRoot) model.storeRoot = dto.storeRoot;
    },
  });
  let revision = 0;
  return {
    slot,
    runtime,
    subscriptions,
    added,
    removed,
    publish(storeRoot: string) {
      callbacks.get(subscriptionKey(ref))?.({
        epoch: "epoch",
        revision: ++revision,
        nativeLayrSettings: { storeRoot, version: null },
      });
      subscriptions.publish(ref, { storeRoot, version: null });
    },
  };
}

describe("observable resource runtime over the subscription owner", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("opens nothing for untracked reads and equal replay fires no reaction", async () => {
    const f = fixture();
    expect(f.slot.current.status).toBe("loading");
    await vi.advanceTimersByTimeAsync(100);
    expect(f.added).not.toHaveBeenCalled();
    const seen: string[] = [];
    const stop = autorun(() => {
      const value = f.slot.current;
      seen.push(
        value.status === "live" || value.status === "stale"
          ? value.data.storeRoot || ""
          : value.status,
      );
    });
    await vi.advanceTimersByTimeAsync(100);
    f.publish("one");
    await vi.advanceTimersByTimeAsync(0);
    expect(seen).toEqual(["loading", "one"]);
    f.publish("one");
    await vi.advanceTimersByTimeAsync(0);
    expect(seen).toEqual(["loading", "one"]);
    f.publish("two");
    await vi.advanceTimersByTimeAsync(0);
    expect(seen).toEqual(["loading", "one", "two"]);
    stop();
    f.runtime.dispose();
    expect(f.slot.current).toEqual({ status: "loading" });
    f.subscriptions.dispose();
  });

  it("marks the last reader stale immediately and reuses transport through grace", async () => {
    const f = fixture();
    const first = autorun(() => f.slot.current);
    await vi.advanceTimersByTimeAsync(100);
    f.publish("one");
    await vi.advanceTimersByTimeAsync(0);
    first();
    expect(f.slot.current.status).toBe("stale");
    expect(f.removed).not.toHaveBeenCalled();
    const second = autorun(() => f.slot.current);
    await vi.advanceTimersByTimeAsync(100);
    expect(f.added).toHaveBeenCalledTimes(1);
    expect(f.slot.current.status).toBe("live");
    second();
    await vi.advanceTimersByTimeAsync(20_000);
    expect(f.removed).toHaveBeenCalledTimes(1);
    f.runtime.dispose();
    f.subscriptions.dispose();
  });

  it("scopes identical resource keys to their own server", async () => {
    const a = fixture(),
      b = fixture();
    const stopA = autorun(() => a.slot.current);
    const stopB = autorun(() => b.slot.current);
    await vi.advanceTimersByTimeAsync(100);
    a.publish("server-a");
    b.publish("server-b");
    await vi.advanceTimersByTimeAsync(0);
    expect(a.slot.current).toMatchObject({
      status: "live",
      data: { storeRoot: "server-a" },
    });
    expect(b.slot.current).toMatchObject({
      status: "live",
      data: { storeRoot: "server-b" },
    });
    stopA();
    stopB();
    a.runtime.dispose();
    b.runtime.dispose();
    a.subscriptions.dispose();
    b.subscriptions.dispose();
  });

  it("reuses A through A to B to A navigation inside the existing grace", async () => {
    const f = fixture();
    const other = { kind: "desktop" } as const;
    const b = f.runtime.slot({
      key: subscriptionKey(other),
      ref: other,
      create: () => ({}),
      apply: () => {},
    });
    const aFirst = autorun(() => f.slot.current);
    await vi.advanceTimersByTimeAsync(100);
    aFirst();
    const stopB = autorun(() => b.current);
    await vi.advanceTimersByTimeAsync(100);
    stopB();
    const aAgain = autorun(() => f.slot.current);
    await vi.advanceTimersByTimeAsync(100);
    expect(f.added).toHaveBeenCalledTimes(2);
    expect(f.removed).not.toHaveBeenCalled();
    aAgain();
    await vi.advanceTimersByTimeAsync(20_000);
    expect(f.removed).toHaveBeenCalledTimes(2);
    f.runtime.dispose();
    f.subscriptions.dispose();
  });

  it("retains data as stale with error detail and clears it at owner disposal", async () => {
    const f = fixture();
    const stop = autorun(() => f.slot.current);
    await vi.advanceTimersByTimeAsync(100);
    f.publish("saved");
    await vi.advanceTimersByTimeAsync(0);
    f.subscriptions.fail(ref, new Error("Disconnected"));
    await vi.advanceTimersByTimeAsync(0);
    expect(f.slot.current).toMatchObject({
      status: "stale",
      data: { storeRoot: "saved" },
      error: { message: "Disconnected" },
    });
    stop();
    f.subscriptions.dispose();
    expect(f.slot.current).toEqual({ status: "loading" });
    f.runtime.dispose();
  });
});

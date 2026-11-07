import { approval } from "../approvalFixtures.ts";
import { autorun, makeObservable, observable, spy } from "mobx";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ResourceVersion } from "../sync/resourceEvents.ts";
import { agentId } from "../generated/observableResources.ts";
import { createServerStore } from "./server.ts";
import { createSubscriptions } from "../sync/subscriptions/module.ts";
import { subscriptionKey } from "../sync/subscriptions/contracts.ts";
import type { ResourceRef } from "../sync/subscriptions/contracts.ts";
import {
  createObservableResourceRuntime,
  MAX_INACTIVE_RESOURCE_FACADES,
} from "./runtime.ts";

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
        nativeLayrSettings: { configured: true, storeRoot, version: null },
      });
      subscriptions.publish(ref, {
        configured: true,
        storeRoot,
        version: null,
      });
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

  it("equal replay produces zero observable model mutations", async () => {
    const f = fixture();
    const stop = autorun(() => f.slot.current);
    await vi.advanceTimersByTimeAsync(100);
    f.publish("same");
    await vi.advanceTimersByTimeAsync(0);
    let mutations = 0;
    const stopSpy = spy((event) => {
      if (
        event.type === "update" ||
        event.type === "add" ||
        event.type === "delete" ||
        event.type === "splice"
      )
        mutations++;
    });
    f.publish("same");
    await vi.advanceTimersByTimeAsync(0);
    expect(mutations).toBe(0);
    stopSpy();
    stop();
    f.runtime.dispose();
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

/** Exercise argument identity without a generated adapter: the generator owns
 * model apply, while this boundary owns observation and slot retention only. */
function keyedFixture() {
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
  type Ref = Extract<ResourceRef, { kind: "processes" }>;
  const get = runtime.keyed<Ref, SettingsModel>({
    create: () => new SettingsModel("initial"),
    apply: () => {},
  });
  const ref = (agentId: string): Ref => ({ kind: "processes", agentId });
  return {
    runtime,
    subscriptions,
    added,
    removed,
    callbacks,
    get,
    ref,
    publish(agentId: string) {
      const version: ResourceVersion = {
        epoch: "keyed",
        revision: 1,
        viewSnapshot: { processes: { processes: [], activities: [] } },
      };
      callbacks.get(subscriptionKey(ref(agentId)))?.(version);
      subscriptions.publish(
        ref(agentId),
        { processes: [], activities: [] },
        version,
      );
    },
    close() {
      runtime.dispose();
      subscriptions.dispose();
    },
  };
}

describe("bounded keyed observable facades", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("canonicalizes argument order, waits 100 ms, and preserves model identity on equal replay", async () => {
    const f = keyedFixture();
    const slot = f.get(f.ref("a"));
    expect(f.get({ agentId: "a", kind: "processes" })).toBe(slot);
    const seen = vi.fn(() => slot.current);
    const stop = autorun(seen);
    await vi.advanceTimersByTimeAsync(99);
    expect(f.added).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(f.added).toHaveBeenCalledOnce();
    f.publish("a");
    await vi.advanceTimersByTimeAsync(0);
    const current = slot.current;
    expect(current.status).toBe("live");
    const renders = seen.mock.calls.length;
    f.publish("a");
    await vi.advanceTimersByTimeAsync(0);
    expect(slot.current).toBe(current);
    expect(seen).toHaveBeenCalledTimes(renders);
    stop();
    expect(slot.current.status).toBe("stale");
    await vi.advanceTimersByTimeAsync(19_999);
    expect(f.removed).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(f.removed).toHaveBeenCalledOnce();
    f.close();
  });

  it("bounds never-observed lookups without acquiring or starting timers", async () => {
    const f = keyedFixture();
    const oldest = f.get(f.ref("oldest"));
    for (let i = 0; i < MAX_INACTIVE_RESOURCE_FACADES; i++)
      f.get(f.ref(String(i)));
    expect(f.get(f.ref("oldest"))).not.toBe(oldest);
    expect(oldest.current).toEqual({ status: "loading" });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(f.added).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
    f.close();
  });

  it("evicts the least recently used unobserved facade, resets held slots, and keeps grace", async () => {
    const f = keyedFixture();
    const oldest = f.get(f.ref("oldest"));
    const stop = autorun(() => oldest.current);
    await vi.advanceTimersByTimeAsync(100);
    f.publish("oldest");
    await vi.advanceTimersByTimeAsync(0);
    stop();
    expect(oldest.current.status).toBe("stale");
    const recent = f.get(f.ref("recent"));
    for (let i = 0; i < MAX_INACTIVE_RESOURCE_FACADES - 2; i++)
      f.get(f.ref(String(i)));
    expect(f.get(f.ref("recent"))).toBe(recent);
    f.get(f.ref("overflow"));
    expect(oldest.current).toEqual({ status: "loading" });
    expect(f.removed).not.toHaveBeenCalled();
    const again = autorun(() => oldest.current);
    await vi.advanceTimersByTimeAsync(100);
    expect(f.added).toHaveBeenCalledTimes(1);
    expect(oldest.current.status).toBe("live");
    again();
    f.close();
  });

  it("shares one inactive facade bound across argument families", () => {
    const f = keyedFixture();
    const old = f.get(f.ref("old"));
    const other = f.runtime.keyed<typeof ref, string | null>({
      create: (dto) => dto.storeRoot,
      apply: (_data, dto) => dto.storeRoot,
    });
    for (let i = 0; i < MAX_INACTIVE_RESOURCE_FACADES - 1; i++)
      f.get(f.ref(String(i)));
    other(ref);
    expect(f.get(f.ref("old"))).not.toBe(old);
    f.close();
  });

  it("pins observed facades and cancels an abandoned short observation", async () => {
    const f = keyedFixture();
    const pinned = f.get(f.ref("pinned"));
    const stop = autorun(() => pinned.current);
    for (let i = 0; i < 2 * MAX_INACTIVE_RESOURCE_FACADES; i++)
      f.get(f.ref(String(i)));
    expect(f.get(f.ref("pinned"))).toBe(pinned);
    await vi.advanceTimersByTimeAsync(50);
    stop();
    await vi.advanceTimersByTimeAsync(100);
    expect(f.added).not.toHaveBeenCalled();
    f.close();
  });

  it("resets a held facade when the subscription owner is disposed", async () => {
    const f = keyedFixture();
    const slot = f.get(f.ref("a"));
    const seen: string[] = [];
    const stop = autorun(() => {
      seen.push(slot.current.status);
    });
    await vi.advanceTimersByTimeAsync(100);
    f.publish("a");
    await vi.advanceTimersByTimeAsync(0);
    f.subscriptions.dispose();
    expect(slot.current).toEqual({ status: "loading" });
    expect(seen.at(-1)).toBe("loading");
    stop();
    f.runtime.dispose();
  });

  it("publishes replacement scalar roots without losing equal-replay isolation", async () => {
    const f = keyedFixture();
    const settings = f.runtime.keyed<typeof ref, string | null>({
      create: (dto) => dto.storeRoot,
      apply: (_model, dto) => dto.storeRoot,
    });
    const slot = settings(ref);
    const seen: (string | null)[] = [];
    const stop = autorun(() => {
      const value = slot.current;
      if (value.status === "live" || value.status === "stale")
        seen.push(value.data);
    });
    await vi.advanceTimersByTimeAsync(100);
    for (const storeRoot of ["one", "one", null]) {
      f.subscriptions.publish(ref, {
        configured: true,
        storeRoot,
        version: null,
      });
      await vi.advanceTimersByTimeAsync(0);
    }
    expect(seen).toEqual(["one", null]);
    stop();
    f.close();
  });
});

describe("generated agent detail facade through the runtime", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("passes a stable model reference to leaves and equal replay mutates nothing", async () => {
    const f = keyedFixture();
    const store = createServerStore(f.subscriptions);
    const slot = store.agentDetail(agentId("worker"));
    expect(store.agentDetail(agentId("worker"))).toBe(slot);
    const parent = vi.fn(() => slot.current);
    const stopParent = autorun(parent);
    await vi.advanceTimersByTimeAsync(100);
    const detailRef = { kind: "agentDetail", agentId: "worker" } as const;
    const publish = async (task: string) => {
      f.callbacks.get(subscriptionKey(detailRef))?.({
        epoch: "detail",
        revision: 1,
        cursor: { workspaceId: "detail", seq: 1 },
        replyKind: "snapshot",
      });
      f.subscriptions.publish(detailRef, { id: "worker", overview: { task } });
      await vi.advanceTimersByTimeAsync(0);
    };
    await publish("First");
    const current = slot.current;
    if (current.status !== "live")
      throw new Error("Expected an applied baseline");
    const model = current.data;
    const leaf = vi.fn(() => model.overview?.task);
    const stopLeaf = autorun(leaf);
    const parentRenders = parent.mock.calls.length;
    let mutations = 0;
    const stopSpy = spy((event) => {
      if (
        event.type === "update" ||
        event.type === "add" ||
        event.type === "delete" ||
        event.type === "splice"
      )
        mutations++;
    });
    await publish("First");
    expect(mutations).toBe(0);
    expect(leaf).toHaveBeenCalledTimes(1);
    stopSpy();
    await publish("Next");
    expect(leaf).toHaveBeenCalledTimes(2);
    expect(leaf.mock.results.at(-1)?.value).toBe("Next");
    expect(parent).toHaveBeenCalledTimes(parentRenders);
    expect(slot.current).toBe(current);
    stopLeaf();
    stopParent();
    for (let i = 0; i < MAX_INACTIVE_RESOURCE_FACADES; i++)
      store.agentDetail(agentId(String(i)));
    expect(slot.current).toEqual({ status: "loading" });
    store.dispose();
    f.close();
  });
});

describe("independent request projections over one state subscription", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());
  it("sidebar builds zero Approval models, readers acquire projections and eviction clears all", async () => {
    const f = keyedFixture();
    const store = createServerStore(f.subscriptions);
    const stops: (() => void)[] = [];
    let approvalAdds = 0;
    const stopSpy = spy((event) => {
      if (
        event.type === "add" &&
        event.object?.constructor.name === "ApprovalModel"
      )
        approvalAdds++;
    });
    try {
      stops.push(autorun(() => store.state().current));
      await vi.advanceTimersByTimeAsync(100);
      const row = approval({
        request: { kind: "monitor_start", monitor: "m", command: "tail" },
      });
      const ref = { kind: "state" } as const;
      f.callbacks.get(subscriptionKey(ref))?.({
        epoch: "state",
        revision: 1,
        cursor: { workspaceId: "state", seq: 1 },
        replyKind: "snapshot",
      });
      f.subscriptions.publish(ref, [
        { collection: "request", id: row.id, value: row },
        { collection: "questionHistory", id: row.id, value: row },
      ]);
      await vi.advanceTimersByTimeAsync(0);
      expect(approvalAdds).toBe(0);
      const requests = store.requests(),
        history = store.questionHistory();
      stops.push(autorun(() => requests.current));
      await vi.advanceTimersByTimeAsync(100);
      const first = requests.current;
      expect(first.status === "live" && first.data.requests.size).toBe(1);
      expect(approvalAdds).toBeGreaterThan(0);
      const requestAdds = approvalAdds;
      expect(history.current.status).toBe("loading");
      stops.push(autorun(() => history.current));
      await vi.advanceTimersByTimeAsync(100);
      expect(approvalAdds).toBeGreaterThan(requestAdds);
      expect(f.added).toHaveBeenCalledTimes(1);
      for (const stop of stops.splice(0)) stop();
      f.subscriptions.dispose();
      expect(requests.current.status).toBe("loading");
      expect(history.current.status).toBe("loading");
      expect(store.state().current.status).toBe("loading");
    } finally {
      stopSpy();
      for (const stop of stops) stop();
      store.dispose();
      f.close();
    }
  });
});

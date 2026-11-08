import { autorun } from "mobx";
import { expect, it, vi } from "vitest";
import { createServerStore } from "./server.ts";
import { createSubscriptions } from "../sync/subscriptions/module.ts";
import { resourceData } from "./runtime.ts";

it("shares transient facade interest and applies inline values without HTTP", async () => {
  vi.useFakeTimers();
  const watch = vi.fn(() => () => {});
  const subscriptions = createSubscriptions({
    watchResourceChanges: watch,
    watchResourceConnection: () => () => {},
  });
  const store = createServerStore(subscriptions);
  const seen: (string | null | undefined)[] = [];
  try {
    expect(store.nativeLayrSettings()).toBe(store.nativeLayrSettings());
    expect(watch).not.toHaveBeenCalled();
    const stop = autorun(() =>
      seen.push(resourceData(store.nativeLayrSettings().current)?.storeRoot),
    );
    await vi.advanceTimersByTimeAsync(100);
    subscriptions.publish(
      { kind: "native-layr-settings" },
      { configured: true, storeRoot: "store", version: null },
    );
    await vi.advanceTimersByTimeAsync(0);
    expect(seen.at(-1)).toBe("store");
    expect(watch).toHaveBeenCalledTimes(1);
    stop();
  } finally {
    store.dispose();
    subscriptions.dispose();
    vi.useRealTimers();
  }
});

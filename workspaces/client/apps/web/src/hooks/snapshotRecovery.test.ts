import { observableFixture } from "../components/observableFixture.tsx";
// @vitest-environment happy-dom
import { createServerServices } from "../servers/serverServices.ts";
import { LocalCollection } from "../sync/localCollection.ts";
import type { AppScope } from "../app/contracts.ts";
import { afterEach, expect, it, vi } from "vitest";
import { reaction } from "mobx";
import { READ_RECOVERY_MAX_ATTEMPTS } from "../sync/readRetry";
import { localTime } from "../local-time";
import { StartupOwner } from "../app/startup/StartupOwner.ts";
const h = {
  effects: [] as (() => void | (() => void))[],
  session: vi.fn<AppScope["services"]["api"]["refreshSession"]>(),
  database: vi.fn<AppScope["services"]["sync"]["syncDatabase"]>(),
  writes: [] as unknown[],
  connection: undefined as
    | Parameters<
        AppScope["services"]["resources"]["watchResourceConnection"]
      >[0]
    | undefined,
};
function mountStartup() {
  const services = createServerServices({
    id: `startup-${crypto.randomUUID()}`,
    origin: "https://startup.fixture",
    label: "Fixture",
  });
  vi.spyOn(services.api, "refreshSession").mockImplementation(h.session);
  vi.spyOn(services.api, "onApiSchemaMismatch").mockReturnValue(() => {});
  vi.spyOn(services.sync, "syncDatabase").mockImplementation(h.database);
  vi.spyOn(services.resources, "watchResourceConnection").mockImplementation(
    (next) => {
      h.connection = next;
      return () => {};
    },
  );
  const models = observableFixture();
  const owner = new StartupOwner(
    {
      services,
      store: models.store,
      screen: { active: false, channel: null },
    },
    { canUpdateRenderer: () => false, acceptUiPreferences: () => {} },
  );
  h.effects.push(() => {
    const status = reaction(
      () => ({
        startupError: owner.startupError,
        projectionError: owner.projectionError,
        transportError: owner.transportError,
      }),
      (value) => h.writes.push(value),
    );
    const stop = owner.start();
    return () => {
      status();
      stop();
      models.store.dispose();
      services.close();
    };
  });
  return owner;
}
let stops: (() => void)[] = [];
afterEach(() => {
  stops.forEach((stop) => stop());
  stops = [];
  h.effects.length = 0;
  h.writes.length = 0;
  vi.clearAllMocks();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
it.each(["credentials", "database"])(
  "bounds %s startup recovery and displays exhaustion",
  async (boundary) => {
    vi.useFakeTimers();
    vi.stubGlobal("navigator", { onLine: true });
    h.database.mockResolvedValue({
      workspaceId: "workspace",
      db: { projections: new LocalCollection("snapshot-test") },
      verifyWorkspace: async () => {},
    });
    h.session.mockResolvedValue({ token: "token" });
    const read = boundary === "credentials" ? h.session : h.database;
    read.mockRejectedValue(new TypeError("unreachable"));
    const hook = mountStartup();
    stops = h.effects.flatMap((effect) => {
      const stop = effect();
      return stop ? [stop] : [];
    });
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    expect(read).toHaveBeenCalledTimes(READ_RECOVERY_MAX_ATTEMPTS);
    expect(h.writes).toContainEqual(
      expect.objectContaining({
        startupError: expect.stringContaining("exhausted"),
      }),
    );
    window.dispatchEvent(new Event("focus"));
    await vi.advanceTimersByTimeAsync(60_000);
    expect(read).toHaveBeenCalledTimes(READ_RECOVERY_MAX_ATTEMPTS);
    h.connection?.("degraded", "open", {});
    h.connection?.("live", "open", {});
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    expect(read).toHaveBeenCalledTimes(READ_RECOVERY_MAX_ATTEMPTS * 2);
    await hook.refresh(true);
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    expect(read).toHaveBeenCalledTimes(READ_RECOVERY_MAX_ATTEMPTS * 3);
  },
);

it.each([true, false])(
  "does not turn the browser offline hint into a startup failure (initiallyOnline=%s)",
  (online) => {
    vi.stubGlobal("navigator", { onLine: online });
    h.database.mockResolvedValue({
      workspaceId: "workspace",
      db: { projections: new LocalCollection("snapshot-test") },
      verifyWorkspace: async () => {},
    });
    h.session.mockResolvedValue({ token: "token" });
    mountStartup();
    stops = h.effects.flatMap((effect) => {
      const stop = effect();
      return stop ? [stop] : [];
    });
    h.connection?.("live", "open", {});
    window.dispatchEvent(new Event("offline"));
    expect(h.writes).not.toContainEqual(
      expect.objectContaining({
        startupError: expect.stringContaining("Offline"),
      }),
    );
    h.connection?.("degraded", "stopped", {});
    expect(h.writes).toContainEqual(
      expect.objectContaining({
        transportError: "Reason unknown. Connection interrupted.",
      }),
    );
    h.connection?.("paused", "open", {});
    expect(h.writes.at(-1)).toEqual(
      expect.objectContaining({ transportError: "" }),
    );
  },
);

it.each([
  ["live", "offline", {}, "No network."],
  ["degraded", "preparing", {}, "Preparing connection…"],
  ["connecting", "preparing", {}, ""],
  [
    "degraded",
    "connecting",
    { failure: "heartbeat-timeout" },
    "Server updates timed out. Reconnecting…",
  ],
  [
    "degraded",
    "waiting-owner",
    { failure: "stream-error" },
    "Connection failed. Waiting for the shared connection…",
  ],
] as const)(
  "uses actual %s health and %s activity for the snapshot warning",
  (health, activity, detail, transportError) => {
    h.database.mockResolvedValue({
      workspaceId: "workspace",
      db: { projections: new LocalCollection("snapshot-test") },
      verifyWorkspace: async () => {},
    });
    h.session.mockResolvedValue({ token: "token" });
    mountStartup();
    stops = h.effects.flatMap((effect) => {
      const stop = effect();
      return stop ? [stop] : [];
    });
    h.connection?.(health, activity, detail);
    if (transportError)
      expect(h.writes).toContainEqual(
        expect.objectContaining({ transportError }),
      );
    else
      expect(h.writes).not.toContainEqual(
        expect.objectContaining({
          transportError: expect.stringContaining("Preparing"),
        }),
      );
  },
);
it("shows the scheduled retry deadline in the snapshot warning", () => {
  h.database.mockResolvedValue({
    workspaceId: "workspace",
    db: { projections: new LocalCollection("snapshot-test") },
    verifyWorkspace: async () => {},
  });
  h.session.mockResolvedValue({ token: "token" });
  mountStartup();
  stops = h.effects.flatMap((effect) => {
    const stop = effect();
    return stop ? [stop] : [];
  });
  const retryAt = 1_792_965_000_000;
  h.connection?.("degraded", "retry-scheduled", {
    failure: "stream-error",
    retryAt,
  });
  expect(h.writes).toContainEqual(
    expect.objectContaining({
      transportError: `Connection failed. Retry scheduled for ${localTime(new Date(retryAt), { hour: "2-digit", minute: "2-digit", second: "2-digit" })}.`,
    }),
  );
});

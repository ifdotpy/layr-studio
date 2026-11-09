// Startup retry budgets exclude the separately observed remote-name transport.
vi.mock("../../servers/serverNames.ts", () => ({
  watchServerName: () => () => {},
}));
import {
  observableFixture,
  setStartupFixture,
} from "../../components/shell/integration-support/observableFixture.tsx";
import { LocalCollection } from "../../sync/localCollection.ts";
// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { reaction } from "mobx";
import { createServerServices } from "../../servers/serverServices.ts";
import { componentSnapshot as snapshotFixture } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { READ_RECOVERY_MAX_ATTEMPTS } from "../../sync/readRetry.ts";
import { StartupOwner } from "./StartupOwner.ts";
import { ScreenSelection } from "./ScreenOwner.ts";
import { createScreenChannel } from "../../servers/screenChannel.ts";

const cleanup: (() => void)[] = [];
afterEach(() => {
  cleanup
    .splice(0)
    .reverse()
    .forEach((stop) => stop());
  vi.useRealTimers();
  vi.restoreAllMocks();
});
function fixture(credentialId = "first") {
  const services = createServerServices({
    id: "same",
    origin: "https://same.test",
    credentialId,
    label: "Same",
  });
  cleanup.push(() => services.close());
  const session = vi
    .spyOn(services.api, "refreshSession")
    .mockResolvedValue({ token: credentialId });
  vi.spyOn(services.sync, "refreshState").mockResolvedValue();
  const models = observableFixture();
  cleanup.push(() => models.store.dispose());
  const database = vi
    .fn<typeof services.sync.syncDatabase>()
    .mockResolvedValue({
      workspaceId: "workspace",
      db: { projections: new LocalCollection("startup-test") },
    });
  let connection: Parameters<
    typeof services.resources.watchResourceConnection
  >[0] = () => {};
  vi.spyOn(services.resources, "watchResourceConnection").mockImplementation(
    (next) => {
      connection = next;
      return () => {};
    },
  );
  vi.spyOn(services.api, "onApiSchemaMismatch").mockReturnValue(() => {});
  const update = vi
    .spyOn(services.api, "automaticallyUpdateRenderer")
    .mockResolvedValue(false);
  const preferences = vi.fn();
  const owner = new StartupOwner(
    {
      services: {
        ...services,
        sync: Object.assign(services.sync, { syncDatabase: database }),
      },
      store: models.store,
      screen: services.screen,
    },
    {
      canUpdateRenderer: () => true,
      acceptUiPreferences: preferences,
    },
  );
  const areas = {
    scope: owner.scope,
  } as import("../ServerAreasProvider.tsx").ServerAreas;
  setStartupFixture(areas, null);
  cleanup.push(owner.dispose);
  return {
    services,
    models,
    preferences,
    owner,
    session,
    database,
    update,
    connection: (...args: Parameters<typeof connection>) => connection(...args),
    accept: (data: { stateDir: string; token?: string }) =>
      setStartupFixture(areas, data),
  };
}
it.each(["session", "database"] as const)(
  "bounds %s retries, then recovers only on explicit retry or reconnect",
  async (boundary) => {
    vi.useFakeTimers();
    const f = fixture();
    f[boundary].mockRejectedValue(new TypeError("unreachable"));
    f.owner.start();
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    expect(f[boundary]).toHaveBeenCalledTimes(READ_RECOVERY_MAX_ATTEMPTS);
    expect(f.owner.error).toContain("exhausted");
    window.dispatchEvent(new Event("focus"));
    await vi.advanceTimersByTimeAsync(60_000);
    expect(f[boundary]).toHaveBeenCalledTimes(READ_RECOVERY_MAX_ATTEMPTS);
    f.connection("degraded", "open", {});
    f.connection("live", "open", {});
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    expect(f[boundary]).toHaveBeenCalledTimes(READ_RECOVERY_MAX_ATTEMPTS * 2);
    f.session.mockResolvedValue({ token: "restored" });
    f.database.mockResolvedValue({
      workspaceId: "restored",
      db: { projections: new LocalCollection("restored") },
    });
    await f.owner.refreshCredentials();
    expect(f.owner.error).toBe("");
  },
);
it("keeps readiness stable through projection updates and ignores old credentials after disposal", async () => {
  const a = fixture("a");
  const b = fixture("b");
  let resolveSession: (
    value: Awaited<ReturnType<typeof a.services.api.refreshSession>>,
  ) => void = () => {};
  a.session.mockReturnValue(
    new Promise((resolve) => {
      resolveSession = resolve;
    }),
  );
  a.owner.start();
  b.owner.start();
  await Promise.resolve();
  a.owner.dispose();
  resolveSession({ token: "obsolete" });
  await Promise.resolve();
  expect(a.owner.isReady).toBe(false);
  expect(b.session).toHaveBeenCalledOnce();
  let readinessChanges = 0;
  cleanup.push(
    reaction(
      () => b.owner.isReady,
      () => readinessChanges++,
    ),
  );
  b.accept(
    Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
  );
  for (let i = 0; i < 100; i++)
    b.accept({
      ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
      token: String(i),
    });
  expect(readinessChanges).toBe(1);
  expect(a.owner.isReady).toBe(false);
});
it("reads activation directly and aborts automatic reload when its scope becomes inactive", async () => {
  const f = fixture();
  const selection = new ScreenSelection(["same", "other"]);
  f.services.screen.connect(
    createScreenChannel("same", () => {}),
    () => selection.selected === "same",
  );
  f.owner.start();
  f.owner.showSchemaMismatch();
  expect(f.update).toHaveBeenCalledOnce();
  const signal = f.update.mock.calls[0]?.[1];
  selection.select("other");
  expect(signal?.aborted).toBe(true);
  selection.select("same");
  expect(f.update).toHaveBeenCalledTimes(2);
  f.owner.dispose();
  expect(f.update.mock.calls[1]?.[1]?.aborted).toBe(true);
});
it("composes separate selection and dialogs for colliding server identities and credential remounts", () => {
  const a = fixture("a");
  const b = fixture("b");
  const first = a.services.areas;
  const second = b.services.areas;
  expect(a.services.areas).toBe(first);
  expect(first).not.toBe(second);
  first.navigation.selectExternal("colliding-agent", {
    requestId: "a-navigation",
  });
  first.settings.openSettings("appearance");
  expect(second.navigation.selectedId).toBeNull();
  expect(second.settings.studioOpen).toBe(false);
  first.dispose();
  expect(a.services.areas).not.toBe(first);
  expect(second.scope.services.owner.credentialId).toBe("b");
});

it("shows a generated state query failure before its first value and reload recovers the real App", async () => {
  const { act, createElement } = await import("react");
  const { createRoot } = await import("react-dom/client");
  const { MantineProvider } = await import("@mantine/core");
  const { default: App } = await import("../../components/shell/App/App.tsx");
  const { ServerAreas, ServerAreasContextProvider } =
    await import("../ServerAreasProvider.tsx");
  const { ServerServicesProvider, ServerStoreProvider } =
    await import("../../components/servers/ServerServicesProvider/ServerServicesProvider.tsx");
  const { createSubscriptions } =
    await import("../../sync/subscriptions/module.ts");
  const { createEntityDataObserver } =
    await import("../../sync/subscriptions/entityData.ts");
  const { createServerStore } = await import("../../store/server.ts");
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.useFakeTimers();
  const projections = new LocalCollection("startup-query-reload");
  vi.spyOn(projections, "find").mockImplementationOnce(() => {
    throw new Error("Generated state query failed");
  });
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  let release = () => {};
  const reload = async () => {
    await act(async () => release());
    const f = fixture();
    const resources = createSubscriptions(
      {
        watchResourceChanges: () => () => {},
        watchResourceConnection: (accept) => {
          accept("live", "open", {});
          return () => {};
        },
      },
      createEntityDataObserver(async () => ({
        workspaceId: "workspace",
        db: { projections },
      })),
    );
    const store = createServerStore(resources);
    const areas = new ServerAreas({
      services: f.services,
      store,
      screen: { active: true, channel: null },
    });
    setStartupFixture(
      areas,
      Object.assign(snapshotFixture(), {
        token: "",
        stateDir: "/fixture",
      }),
    );
    release = () => {
      areas.dispose();
      store.dispose();
      resources.dispose();
    };
    await act(async () => {
      root.render(
        createElement(MantineProvider, {
          children: createElement(ServerServicesProvider, {
            services: f.services,
            children: createElement(ServerStoreProvider, {
              value: store,
              children: createElement(ServerAreasContextProvider, {
                value: areas,
                children: createElement(App),
              }),
            }),
          }),
        }),
      );
    });
    await act(async () => vi.advanceTimersByTimeAsync(150));
    return areas;
  };
  cleanup.push(() => {
    act(() => root.unmount());
    release();
    host.remove();
    vi.unstubAllGlobals();
  });
  const first = await reload();
  expect(first.startup.isReady).toBe(true);
  expect(first.state).toBeUndefined();
  expect(first.scope.store.state().current).toMatchObject({
    status: "error",
    error: new Error("Generated state query failed"),
  });
  expect(host.querySelector('[role="alert"]')?.textContent).toContain(
    "Generated state query failed",
  );
  const retry = host.querySelector<HTMLAnchorElement>('a[href="/"]');
  expect(retry?.textContent).toBe("Reload Studio");
  // The browser reloads the document; this DOM harness remounts its owner tree.
  if (!retry) throw new Error("Missing startup recovery link");
  let recovery: ReturnType<typeof reload> | undefined;
  retry.addEventListener(
    "click",
    (event) => {
      event.preventDefault();
      recovery = reload();
    },
    { once: true },
  );
  retry.click();
  const restored = await recovery;
  expect(restored?.state).toBeDefined();
  expect(host.querySelector('[aria-label="Studio startup"]')).toBeNull();
  expect(host.textContent).not.toContain("Generated state query failed");
});

it("delivers generated preference field updates only after the durable baseline is ready", () => {
  const f = fixture();
  f.owner.start();
  const fields = { theme: { timestamp: 1, writer: "fixture", value: "dark" } };
  f.models.apply([
    { collection: "uiPreferences", id: "user", value: { fields } },
  ]);
  expect(f.preferences).not.toHaveBeenCalled();
  f.accept({ stateDir: "/fixture" });
  expect(f.owner.creationScope).toBe("/fixture");
  expect(f.preferences).toHaveBeenLastCalledWith({
    user: { fields },
    server: undefined,
  });
  const next = { theme: { ...fields.theme, timestamp: 2, value: "light" } };
  f.models.apply([
    { collection: "uiPreferences", id: "user", value: { fields: next } },
  ]);
  expect(f.preferences).toHaveBeenLastCalledWith({
    user: { fields: next },
    server: undefined,
  });
});

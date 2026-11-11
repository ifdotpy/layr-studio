import { ServerAreasContextProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { componentSnapshot as snapshotFixture } from "../../conversation/ComposerAttachments/componentTestFixtures.ts";
import { accountRows } from "../../../servers/AccountsPanelState.ts";
import Workspace from "../../shell/Workspace/Workspace.tsx";
// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { autorun } from "mobx";
import { observer } from "mobx-react-lite";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import {
  createModelsMirror,
  createStateMirror,
} from "../../../generated/observableResources.ts";
import { createServerStore } from "../../../store/server.ts";
import { createSubscriptions } from "../../../sync/subscriptions/module.ts";
import { createWorkerCatalog } from "../../agents/support/WorkerModelPicker.tsx";
import ServerAccountsPanel from "../../servers/ServerAccountsPanel/ServerAccountsPanel.tsx";
import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { ModelWhitelistSettings } from "../../settings/models/ModelWhitelistSettings/ModelWhitelistSettings.tsx";
import { conversationActionsFixture } from "../../shell/integration-support/observableFixture.tsx";
import { frameAccounts } from "../Accounts/Accounts.tsx";

vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => ({ get: vi.fn() }),
  }),
);
vi.mock("../../shell/WorkspacePanelHost/WorkspacePanelHost.tsx", () => ({
  WorkspacePanelHost: ({ children }: { children: import("react").ReactNode }) =>
    children,
}));
vi.mock("@mantine/core", async (original) => ({
  ...(await original<typeof import("@mantine/core")>()),
  Textarea: () => createElement("textarea"),
}));
const noop = () => {};
function fixture() {
  const state = createStateMirror();
  const models = createModelsMirror();
  const resources = createSubscriptions({
    watchResourceChanges: () => noop,
    watchResourceConnection: (callback) => {
      callback("live", "open", {});
      return noop;
    },
  });
  const runtime = createServerStore(resources);
  const store = {
    ...runtime,
    state: () => ({
      current: { status: "live" as const, data: state.current },
    }),
    models: () => ({
      current: { status: "live" as const, data: models.current },
    }),
  };
  const areas = conversationActionsFixture(undefined, store);
  return {
    areas,
    state,
    models,
    store,
    close() {
      areas.scope.services.close();
      runtime.dispose();
      resources.dispose();
    },
  };
}
const model = (name: string) => ({
  collection: "model" as const,
  id: "account:" + name,
  value: {
    accountKey: "default",
    model: name,
    provider: "codex" as const,
    catalogPosition: 0,
  },
});
const summary = {
  collection: "accounts" as const,
  id: "current",
  value: {
    accounts: [],
    archivedAccounts: [],
    defaultAccountGroups: [],
    defaultAccountKey: "default",
    supportsDelete: true,
    supportsDisconnect: true,
  },
};
afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});
it("retains normalized model rows across equal refreshes and isolates the server owner", () => {
  const first = fixture(),
    second = fixture();
  const reader = { scope: "same", read: vi.fn() };
  first.models.apply([model("gpt-6-astra")]);
  second.models.apply([model("gpt-6-luna")]);
  const view = createWorkerCatalog(first.store, reader, "default", false);
  const other = createWorkerCatalog(second.store, reader, "default", false);
  const seen: unknown[] = [];
  const stop = autorun(() => seen.push(view.models));
  const before = view.models;
  first.models.apply([model("gpt-6-astra")]);
  first.state.apply([summary]);
  expect(view.models).toBe(before);
  expect(view.models[0]).toBe(before[0]);
  expect(seen).toHaveLength(1);
  expect(other.models[0]?.model).toBe("gpt-6-luna");
  first.models.apply([model("gpt-6-sol")]);
  expect(view.models).not.toBe(before);
  expect(seen).toHaveLength(2);
  stop();
  first.close();
  second.close();
});
it("retains the generated summary identity on an unchanged publication", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const f = fixture();
  f.state.apply([summary]);
  const snapshots: unknown[] = [];
  const accounts = f.areas;
  const Probe = observer(function Probe() {
    snapshots.push(accounts.accounts);
    return null;
  });
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  await act(async () =>
    root.render(
      createElement(
        ServerStoreProvider,
        { value: f.store },
        createElement(Probe),
      ),
    ),
  );
  const before = snapshots[0];
  await act(async () => f.state.apply([summary]));
  expect(snapshots).toHaveLength(1);
  expect(snapshots[0]).toBe(before);
  await act(async () => root.unmount());
  f.close();
});
it("updates an actual model consumer when generated models change", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal(
    "matchMedia",
    vi.fn(() => ({
      matches: false,
      addEventListener: noop,
      removeEventListener: noop,
    })),
  );
  const f = fixture();
  f.models.apply([model("gpt-6-astra")]);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  await act(async () =>
    root.render(
      createElement(
        ServerStoreProvider,
        { value: f.store },
        createElement(
          MantineProvider,
          null,
          createElement(
            ServerAreasContextProvider,
            { value: f.areas },
            createElement(ModelWhitelistSettings),
          ),
        ),
      ),
    ),
  );
  expect(host.textContent).toContain("Astra");
  await act(async () => f.models.apply([model("gpt-6-luna")]));
  expect(host.textContent).toContain("Luna");
  expect(host.textContent).not.toContain("Astra");
  await act(async () => root.unmount());
  f.close();
});

it("updates an open Profiles draft when its generated catalog changes", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal("visualViewport", {
    addEventListener: noop,
    removeEventListener: noop,
  });
  vi.stubGlobal(
    "matchMedia",
    vi.fn(() => ({
      matches: false,
      addEventListener: noop,
      removeEventListener: noop,
    })),
  );
  const f = fixture();
  const getProfiles = vi
    .spyOn(f.areas.scope.services.api, "get")
    .mockResolvedValue({ profiles: [] });
  f.models.apply([model("gpt-6-astra")]);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const data = Object.assign(snapshotFixture([]), {
    token: "",
    stateDir: "/fixture",
  });
  data.stateDir = "profile-draft";
  await act(async () =>
    root.render(
      createElement(
        ServerStoreProvider,
        { value: f.store },
        createElement(
          MantineProvider,
          null,
          createElement(
            ServerAreasContextProvider,
            { value: f.areas },
            createElement(Workspace, {
              opened: true,
              initialSection: "profiles",
              onClose: noop,
              onSelect: noop,
              notify: noop,
            }),
          ),
        ),
      ),
    ),
  );
  expect(getProfiles).toHaveBeenCalledWith(
    "/api/profiles",
    expect.objectContaining({ signal: expect.any(AbortSignal) }),
  );
  const add = [...document.querySelectorAll("button")].find(
    (button) => button.textContent === "New profile",
  );
  expect(add).toBeDefined();
  await act(async () => add!.click());
  expect(document.body.textContent).toContain("Astra");
  await act(async () => f.models.apply([model("gpt-6-luna")]));
  expect(document.body.textContent).toContain("Luna");
  expect(document.body.textContent).not.toContain("Astra");
  await act(async () => root.unmount());
  f.close();
});

it("keeps a missing generated accounts summary loading and returns its exact model when present", () => {
  const f = fixture();
  const view = f.areas;
  try {
    expect(view.accounts).toBeUndefined();
    f.state.apply([summary]);
    expect(view.accounts).toBe(f.state.current.accountsSummary.get("current"));
  } finally {
    f.close();
  }
});

it("updates settings account names, defaults and status from incremental generated summaries without a refresh", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    addListener: noop,
    removeListener: noop,
    addEventListener: noop,
    removeEventListener: noop,
    dispatchEvent: () => true,
    media: "",
    onchange: null,
  }));
  const f = fixture();
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const get = vi.spyOn(f.areas.scope.services.api, "get");
  const server = {
    id: "local",
    label: "Fixture",
    origin: "https://fixture.test",
  };
  const account = (id: string, label: string, status: "ready" | "error") => ({
    id,
    label,
    status,
    provider: "codex" as const,
    home: "/fixture/" + id,
    source: "fixture",
    email: id + "@example.test",
  });
  const publish = (
    label: string,
    status: "ready" | "error",
    defaultAccountKey: string,
  ) =>
    f.state.apply([
      {
        ...summary,
        value: {
          ...summary.value,
          accounts: [
            account("one", label, status),
            account("two", "Other", "ready"),
          ],
          defaultAccountKey,
        },
      },
    ]);
  const Screen = observer(function Screen() {
    const accounts = f.areas.accounts;
    return accounts
      ? createElement(ServerAccountsPanel, {
          servers: [server],
          rows: accountRows(
            {
              servers: [server],
              accountsByServer: { local: frameAccounts(accounts) },
              statuses: { local: "live" },
              startAccount: noop,
              accountAction: noop,
            },
            f.store,
          ),
          startAccount: noop,
        })
      : createElement("p", { role: "status" }, "Loading accounts…");
  });
  try {
    await act(async () =>
      root.render(
        createElement(
          ServerStoreProvider,
          { value: f.store },
          createElement(
            MantineProvider,
            { env: "test" },
            createElement(Screen),
          ),
        ),
      ),
    );
    expect(host.textContent).toContain("Loading accounts");
    expect(host.textContent).not.toContain("No accounts found");
    await act(async () => publish("Original", "ready", "one"));
    const generated = f.areas.accounts;
    const row = () =>
      host.querySelector('[data-account-identity="codex:one@example.test"]');
    expect(row()?.textContent).toContain("Original");
    expect(row()?.textContent).toContain("Default Fixture");
    await act(async () => publish("Renamed", "error", "two"));
    expect(f.areas.accounts).toBe(generated);
    expect(row()?.textContent).toContain("Renamed");
    expect(row()?.textContent).not.toContain("Original");
    expect(row()?.textContent).not.toContain("Default Fixture");
    expect(row()?.textContent).toContain("Sign-in check failed");
    expect(
      host.querySelector('[data-account-identity="codex:two@example.test"]')
        ?.textContent,
    ).toContain("Default Fixture");
    expect(get).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
    f.close();
  }
});

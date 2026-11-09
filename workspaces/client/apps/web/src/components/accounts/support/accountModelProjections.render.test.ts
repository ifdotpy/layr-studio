import Workspace from "../../shell/Workspace/Workspace.tsx";
import { componentSnapshot as snapshotFixture } from "../../conversation/ComposerAttachments/componentTestFixtures.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { autorun } from "mobx";
import { afterEach, expect, it, vi } from "vitest";
import {
  createModelsMirror,
  createStateMirror,
} from "../../../generated/observableResources.ts";
import { createServerStore } from "../../../store/server.ts";
import { createSubscriptions } from "../../../sync/subscriptions/module.ts";
import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { ModelWhitelistSettings } from "../../settings/models/ModelWhitelistSettings/ModelWhitelistSettings.tsx";
import { createWorkerCatalog } from "../../agents/support/WorkerModelPicker.tsx";
import { createAccountsView } from "../Accounts/Accounts.tsx";

vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => ({ get: vi.fn() }),
  }),
);
vi.mock("../../shell/Workspace/useWorkspaceResource.ts", () => ({
  useWorkspaceResource: () => ({
    data: { profiles: [] },
    error: "",
    loading: false,
  }),
}));
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
  return {
    state,
    models,
    store,
    close() {
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
it("retains the account view object and full login list on an unchanged refresh", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const f = fixture();
  f.state.apply([summary]);
  const snapshots: unknown[] = [];
  const accounts = createAccountsView(
    f.store,
    async () => {
      throw new Error("Unexpected account read");
    },
    "workspace",
  );
  const Probe = observer(function Probe() {
    snapshots.push(accounts.data);
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
          createElement(ModelWhitelistSettings),
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

it("retains account identities across identical explicit HTTP refreshes", async () => {
  const f = fixture();
  f.state.apply([summary]);
  const result = { ...summary.value, logins: [] };
  const get = vi.fn().mockImplementation(async () => structuredClone(result));
  const view = createAccountsView(f.store, get, "workspace");
  const seen: unknown[] = [];
  const stop = autorun(() => seen.push(view.data));
  await view.refresh();
  const before = view.data;
  const renders = seen.length;
  await view.refresh();
  expect(view.data).toBe(before);
  expect(view.data.accounts).toBe(before.accounts);
  expect(view.data.logins).toBe(before.logins);
  expect(seen).toHaveLength(renders);
  get.mockResolvedValueOnce({
    ...result,
    defaultAccountKey: "readback-only-change",
  });
  await view.refresh();
  expect(view.data.accounts).toBe(before.accounts);
  stop();
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

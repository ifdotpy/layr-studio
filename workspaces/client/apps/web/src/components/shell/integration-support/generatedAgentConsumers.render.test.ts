import * as usageApi from "../../../servers/net/api.ts";
import { NewChatModelStore } from "../../settings/NewChatModelSettings/NewChatModelStore.ts";
import { createProjectPeersStore } from "../../projects/ProjectPeers/ProjectPeersStore.ts";
import { RequestAreaOwner } from "../../conversation/questions/RequestArea/RequestArea.tsx";
import * as requestApi from "../../../servers/net/api.ts";
import { serverLocalStorage } from "../../../servers/storage.ts";
import { createRequestsMirror } from "../../../generated/observableResources.ts";
import CodeBlock from "../CodeBlock/CodeBlock.tsx";
import { createServerServices } from "../../../servers/serverServices.ts";
import NewChatModelSettings from "../../settings/NewChatModelSettings/NewChatModelSettings.tsx";
import { observable, runInAction, action, flow, when } from "mobx";
import {
  ServerAreasContextProvider,
  type ServerAreas,
} from "../../../app/ServerAreasProvider.tsx";
import {
  ServerStoreProvider,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createRoot } from "react-dom/client";
import { WorkerError } from "../../agents/worker/WorkerError/WorkerError.tsx";
import { NativeModelActions } from "../../conversation/NativeModelActions/NativeModelActions.tsx";
import { ConversationProvider } from "../../conversation/support/navigation.ts";
import { ExecutionSummaryTrigger } from "../../agents/ExecutionSummaryTrigger/ExecutionSummaryTrigger.tsx";
import { ClaudeSettings } from "../../settings/ClaudeSettings/ClaudeSettings.tsx";
import TurnHistory from "../../conversation/transcript/TurnHistory/TurnHistory.fixture.tsx";
import { componentSnapshot as snapshotFixture } from "../../conversation/ComposerAttachments/componentTestFixtures.ts";
// @vitest-environment happy-dom
import {
  createElement,
  act,
  Profiler,
  Fragment,
  useState,
  type ComponentProps,
  type ReactNode,
} from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider, Menu } from "@mantine/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

it("keeps code expanded when its source changes", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"),
    root = createRoot(host);
  const render = (source: string) =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          {},
          createElement(CodeBlock, { source }),
        ),
      ),
    );
  try {
    await render("first");
    await act(async () =>
      [...host.querySelectorAll("button")]
        .find((button) => button.textContent === "Expand code")
        ?.click(),
    );
    await render("second");
    expect(host.querySelector("pre")?.dataset.expanded).toBe("true");
  } finally {
    await act(async () => root.unmount());
  }
});
import {
  AgentMode,
  AccountStatus,
  AgentSource,
  AgentStatus,
  AgentPermissionMode,
  ProviderKind,
} from "../../../generated/api.ts";
import {
  observableFixture,
  conversationActionsFixture,
} from "./observableFixture.tsx";
import { required } from "../../conversation/ComposerAttachments/componentTestFixtures.ts";
import { ChatSettingsFixture as ChatSettingsDialog } from "../../../app/settings/ChatSettingsFixture.tsx";
import { UnifiedAgentSettings } from "../../agents/UnifiedAgentSettings/UnifiedAgentSettings.tsx";
import { LiveAgentPhase } from "../../agents/AgentPhase/AgentPhase.tsx";
import {
  UsageFixture as Usage,
  createUsageFixture,
  type UsageFixtureInputs,
} from "../../../app/settings/UsageFixture.tsx";
import Requests, {
  RequestArea,
} from "../../conversation/questions/Requests/Requests.tsx";
import { approval } from "../../../approvalFixtures.ts";
import type { AgentSetupPickerProps } from "../../agents/AgentSetupPicker/AgentSetupPicker.tsx";

const ownedUsageFixtures: ReturnType<typeof createUsageFixture>[] = [];
function usageElement(
  parent: ReturnType<typeof useServerStore>,
  props: UsageFixtureInputs,
) {
  const owner = createUsageFixture(parent, usageApi, props);
  owner.start();
  ownedUsageFixtures.push(owner);
  return createElement(Usage, { ...props, owner });
}
const ownedAreas: ServerAreas[] = [];
const applyFixtureCatalogs = action((areas: ServerAreas) => {
  for (const [key, fallback] of areas.agentsUi.workerCatalogs)
    areas.agentsUi.workerCatalogs.set(key, {
      ...fallback,
      get models() {
        return h.catalog
          ? h.catalog.models.map((row) => ({ ...row, serviceTiers: [] }))
          : fallback.models;
      },
      get loading() {
        return (h.catalog || fallback).loading;
      },
      get error() {
        return (h.catalog || fallback).error;
      },
      retry: flow(function* () {
        yield (h.catalog || fallback).retry();
      }),
      dispose: () => fallback.dispose(),
    });
});
function OwnedUnifiedAgentSettings(
  props: ComponentProps<typeof UnifiedAgentSettings>,
) {
  const store = useServerStore();
  const [areas] = useState(() => {
    const owner = conversationActionsFixture(undefined, store);
    ownedAreas.push(owner);
    return owner;
  });
  return createElement(
    ServerAreasContextProvider,
    { value: areas },
    createElement(UnifiedAgentSettings, props),
  );
}
afterEach(() => {
  for (const owner of ownedUsageFixtures.splice(0)) owner.dispose();
  for (const areas of ownedAreas.splice(0)) {
    areas.dispose();
    areas.scope.services.close();
  }
});

const h = vi.hoisted(() => ({
  catalog: null as null | {
    models: {
      model: string;
      provider: "codex";
      supportedReasoningEfforts: { reasoningEffort: string }[];
    }[];
    loading: boolean;
    error: string;
    retry: () => void;
  },
  pickers: [] as AgentSetupPickerProps[],
  sync: { subscribeStateProjection: () => () => {} },
  api: {
    saved: (_key: string, initial: unknown) => initial,
    save: () => {},
    post: vi.fn(),
    get: vi.fn(),
    command: {},
  },
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => {
    const actual =
      await original<
        typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
      >();
    return {
      ...actual,
      useServerApi: () => h.api,
      useServerSync: () => actual.useServerServices()?.sync ?? h.sync,
    };
  },
);
vi.mock("@mantine/core", async (original) => {
  const actual = await original<typeof import("@mantine/core")>();
  const children = ({ children }: { children?: ReactNode }) =>
    createElement(Fragment, null, children);
  return {
    ...actual,
    Modal: Object.assign(
      ({ children }: { children?: ReactNode }) =>
        createElement(Fragment, null, children),
      actual.Modal,
    ),
    Popover: Object.assign(
      ({ children }: { children?: ReactNode }) =>
        createElement(Fragment, null, children),
      actual.Popover,
      { Target: children, Dropdown: children },
    ),
  };
});
vi.mock("../../accounts/Accounts/Accounts.tsx", async (original) => ({
  ...(await original<typeof import("../../accounts/Accounts/Accounts.tsx")>()),
  default: ({
    renderPicker,
  }: {
    renderPicker: (select: () => void, disabled: boolean) => ReactNode;
  }) => renderPicker(() => {}, false),
  AccountTransferStatus: () => null,
}));
vi.mock("../../accounts/AccountSignInNotice/AccountSignInNotice.tsx", () => ({
  default: ({ errors }: { errors: (string | undefined)[] }) =>
    createElement("output", null, errors.filter(Boolean).join(" ")),
}));
vi.mock("../../agents/support/WorkerModelPicker.tsx", async (original) => {
  const actual =
    await original<
      typeof import("../../agents/support/WorkerModelPicker.tsx")
    >();
  return {
    ...actual,
    createWorkerCatalog: (
      ...args: Parameters<typeof actual.createWorkerCatalog>
    ) => {
      const fallback = actual.createWorkerCatalog(...args);
      return {
        get active() {
          return fallback.active;
        },
        get pending() {
          return fallback.pending;
        },
        get retryError() {
          return fallback.retryError;
        },
        dispose: () => fallback.dispose(),
        get models() {
          return (h.catalog || fallback).models;
        },
        get loading() {
          return (h.catalog || fallback).loading;
        },
        get error() {
          return (h.catalog || fallback).error;
        },
        retry: () => (h.catalog || fallback).retry(),
      };
    },
    workerCatalog: (...args: Parameters<typeof actual.workerCatalog>) =>
      h.catalog || actual.workerCatalog(...args),
    useWorkerModels: (...args: Parameters<typeof actual.useWorkerModels>) => {
      const catalog = actual.useWorkerModels(...args);
      return (args[1] && h.catalog) || catalog;
    },
  };
});
vi.mock("../../conversation/TokenRate/TokenRate.tsx", () => ({
  default: () => null,
}));
vi.mock("../../agents/AgentSetupPicker/AgentSetupPicker.tsx", () => ({
  AgentSetupPicker: (props: AgentSetupPickerProps) => {
    h.pickers.push(props);
    return createElement("output", null, props.model, " ", props.effort);
  },
}));
const catalog = { models: [], loading: false, error: "", retry: () => {} };
const accounts: NonNullable<
  ComponentProps<typeof UnifiedAgentSettings>["state"]
> = {
  accounts: [],
  archivedAccounts: [],
  defaultAccountKey: "default",
  defaultAccountGroups: [],
  supportsDelete: true,
  supportsDisconnect: true,
};
const record = {
  id: "lead",
  source: AgentSource.managed,
  provider: ProviderKind.codex,
  isLead: true,
  status: AgentStatus.idle,
  threadId: "thread",
  epoch: 1,
  model: "gpt-6-astra",
  concurrency: 7,
  subagentConcurrencyVersion: 2,
  agentModeRevision: 3,
  permissionMode: AgentPermissionMode.bypassPermissions,
  effort: "xhigh",
  workerDefaults: {
    model: "worker-sentinel",
    effort: "medium",
    fastMode: false,
    delegationDepth: 2,
  },
  reviewDefaults: { model: "review-sentinel", effort: "high", fastMode: false },
  workerExcludedAccountKeys: ["excluded-worker"],
  reviewerExcludedAccountKeys: ["excluded-review"],
  contextUsage: { tokens: 250, window: 1000 },
  compactions: 4,
};
function render(
  fixture: ReturnType<typeof observableFixture>,
  view: ReactNode,
) {
  const areas = conversationActionsFixture(undefined, fixture.store);
  ownedAreas.push(areas);
  return renderToStaticMarkup(
    createElement(
      MantineProvider,
      { env: "test" },
      createElement(
        fixture.Provider,
        null,
        createElement(ServerAreasContextProvider, { value: areas }, view),
      ),
    ),
  );
}
beforeEach(() => {
  h.pickers = [];
  h.catalog = null;
  h.api.post.mockReset();
});

it("reads generated team concurrency and defaults through ChatSettingsDialog", () => {
  const f = observableFixture([record]);
  const agent = required(f.mirror.current.agents.get("lead"));
  expect("concurrency" in agent).toBe(false);
  const html = render(
    f,
    createElement(ChatSettingsDialog, {
      settingsOpen: true,
      accountModalOpen: false,
      setSettingsOpen: () => {},
      agent,
      lead: agent,
      accounts,
      notify: () => {},
      agentModels: catalog,
      data: Object.assign(snapshotFixture([]), {
        token: "",
        stateDir: "/fixture",
      }),
      refresh: async () => {},
      workspaceId: "workspace",
      creating: false,
      newChat: () => {},
      project: () => {},
      setClaudeLoginKey: () => {},
    }),
  );
  expect(html).toContain('value="7"');
  expect(html).toContain('value="2" selected=""');
  const host = document.createElement("div");
  host.innerHTML = html;
  const full = required(
    host.querySelector<HTMLInputElement>('input[value="full"]'),
  );
  expect(full.checked).toBe(true);
  expect(full.disabled).toBe(false);
  f.store.dispose();
});

it.each([
  {
    role: "orchestrator" as const,
    model: "",
    effort: "xhigh",
    excluded: undefined,
  },
  {
    role: "worker" as const,
    model: "worker-sentinel",
    effort: "medium",
    excluded: ["excluded-worker"],
  },
  {
    role: "review" as const,
    model: "review-sentinel",
    effort: "high",
    excluded: ["excluded-review"],
  },
])(
  "reads generated $role effort and exclusions through UnifiedAgentSettings",
  ({ role, model, effort, excluded }) => {
    const f = observableFixture([record]);
    const agent = required(f.mirror.current.agents.get("lead"));
    render(
      f,
      createElement(OwnedUnifiedAgentSettings, {
        agent,
        initialRole: role,
        state: accounts,
        notify: () => {},
        catalog,
      }),
    );
    const picker = required(h.pickers.find((p) => p.role === role));
    expect(picker.effort).toBe(effort);
    if (model) expect(picker.model).toBe(model);
    expect(picker.excludedKeys).toEqual(excluded);
    f.store.dispose();
  },
);

it("keeps a native thread block visible through the generated phase caller", () => {
  const f = observableFixture([
    {
      ...record,
      status: AgentStatus.running,
      nativeThreadBlock: {
        threadId: "thread",
        error: { message: "Safety block" },
      },
    },
  ]);
  const html = render(
    f,
    createElement(LiveAgentPhase, { agentId: "lead", connection: null }),
  );
  expect(html).toContain('data-phase="blocked"');
  f.store.dispose();
});

it("keeps generated request owners blocked for the same native thread", () => {
  const f = observableFixture([
    {
      ...record,
      nativeThreadBlock: {
        threadId: "thread",
        error: { message: "Safety block" },
      },
    },
  ]);
  const request = approval({
    request: { kind: "exec", command: "pwd", cwd: "/", reason: null },
    params: { threadId: "thread" },
  });
  const requests = createRequestsMirror();
  requests.apply([{ collection: "request", id: request.id, value: request }]);
  const store = {
    ...f.store,
    requests: () => ({
      current: { status: "live" as const, data: requests.current },
    }),
  };
  const requestOwner = new RequestAreaOwner(
    {
      api: () => requestApi,
      store,
      storage: serverLocalStorage,
      server: "local",
    },
    () => ({ scope: "owner", notify() {} }),
  );
  requestOwner.start();
  const html = render(
    f,
    createElement(
      ServerStoreProvider,
      { value: store },
      createElement(
        RequestArea,
        { owner: requestOwner },
        createElement(Requests, { all: true }),
      ),
    ),
  );
  requestOwner.dispose();
  expect(html).toContain("This chat stopped as a precaution");
  f.store.dispose();
});

it("reads generated context usage and compactions through Usage", () => {
  const f = observableFixture([record]);
  const html = render(
    f,
    usageElement(f.store, {
      agent: required(f.mirror.current.agents.get("lead")),
      stateDir: "owner",
      limits: null,
      reload: () => {},
    }),
  );
  expect(html).toContain("Context 25%");
  expect(html).toContain("Compacted 4 times.");
  f.store.dispose();
});

it("reads worker and reviewer defaults through the composer summary trigger", () => {
  const f = observableFixture([record]);
  const agent = required(f.mirror.current.agents.get("lead"));
  const html = render(
    f,
    createElement(ExecutionSummaryTrigger, {
      agent,
      team: [agent],
      onOpen: () => {},
    }),
  );
  expect(html).toContain("worker-sentinel");
  expect(html).toContain("review-sentinel");
  f.store.dispose();
});
it("reads native detail errors through Claude settings", () => {
  const f = observableFixture([
    {
      ...record,
      provider: ProviderKind.claude,
      nativeStatus: { error: "native-only-error" },
    },
  ]);
  const html = render(
    f,
    createElement(ClaudeSettings, {
      agent: required(f.mirror.current.agents.get("lead")),
      onSignIn: () => {},
    }),
  );
  expect(html).toContain("native-only-error");
  f.store.dispose();
});
it("reads a native turn error through the real transcript history caller", () => {
  const f = observableFixture([
    {
      ...record,
      status: AgentStatus.failed,
      nativeTurnError: {
        turnId: "turn",
        error: { message: "native-turn-only-error" },
      },
    },
  ]);
  const html = render(
    f,
    createElement(TurnHistory, {
      agent: required(f.mirror.current.agents.get("lead")),
      enabled: true,
      storageKey: "owner",
      onJump: () => {},
      items: [
        {
          id: "answer",
          role: "assistant",
          text: "Answer",
          turnId: "turn",
          turnStatus: "failed",
        },
      ],
      renderMessage: (item) =>
        createElement(
          "span",
          null,
          item.nativeError ? JSON.stringify(item.nativeError) : item.text,
        ),
    }),
  );
  expect(html).toContain("native-turn-only-error");
  f.store.dispose();
});

it("guards the App native action menu with generated summary and detail", () => {
  const f = observableFixture([
    {
      ...record,
      nativeThreadBlock: {
        threadId: "thread",
        error: { message: "Safety block" },
      },
    },
  ]);
  const html = render(
    f,
    createElement(
      ConversationProvider,
      { value: conversationActionsFixture() },
      createElement(
        Menu,
        { opened: true, withinPortal: false },
        createElement(
          Menu.Target,
          null,
          createElement("button", null, "Actions"),
        ),
        createElement(
          Menu.Dropdown,
          null,
          createElement(NativeModelActions, {
            agent: required(f.mirror.current.agents.get("lead")),
          }),
        ),
      ),
    ),
  );
  const host = document.createElement("div");
  host.innerHTML = html;
  expect(
    required(host.querySelector<HTMLButtonElement>('[data-action="compact"]'))
      .disabled,
  ).toBe(true);
  f.store.dispose();
});

it("keeps worker error and native menu leaves isolated from detail activity", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const initial = {
    ...record,
    error: "Stored error",
    activity: { phase: "thinking" },
    nativeThreadBlock: {
      threadId: "thread",
      error: { message: "Safety block" },
    },
  };
  const f = observableFixture([initial]);
  const agent = required(f.mirror.current.agents.get("lead"));
  const detail = required(f.details.get("lead"));
  const commits = new Map<string, number>();
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const count = (id: string) => commits.set(id, (commits.get(id) || 0) + 1);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(
            f.Provider,
            null,
            createElement(
              Profiler,
              { id: "error", onRender: () => count("error") },
              createElement(WorkerError, { agent }),
            ),
            createElement(
              ConversationProvider,
              { value: conversationActionsFixture() },
              createElement(
                Menu,
                { opened: true, withinPortal: false },
                createElement(
                  Menu.Target,
                  null,
                  createElement("button", null, "Actions"),
                ),
                createElement(
                  Menu.Dropdown,
                  null,
                  createElement(
                    Profiler,
                    { id: "menu", onRender: () => count("menu") },
                    createElement(NativeModelActions, { agent }),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    commits.clear();
    await act(async () =>
      detail.apply({ ...initial, activity: { phase: "writing" } }),
    );
    expect(commits.get("error") || 0).toBe(0);
    expect(commits.get("menu") || 0).toBe(0);
    await act(async () =>
      detail.apply({
        ...initial,
        error: "Changed error",
        activity: { phase: "writing" },
      }),
    );
    expect(commits.get("error") || 0).toBeGreaterThan(0);
    expect(commits.get("menu") || 0).toBe(0);
  } finally {
    await act(async () => root.unmount());
    f.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});

it("updates settings, context usage and native actions when the owning detail slot arrives and changes", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const f = observableFixture([record]);
  const agent = required(f.mirror.current.agents.get("lead"));
  const detail = required(f.details.get("lead"));
  type Current = ReturnType<typeof f.store.agentDetail>["current"];
  const current = observable.box<Current>(
    { status: "loading" },
    { deep: false },
  );
  const store = {
    ...f.store,
    agentDetail: () => ({
      get current() {
        return current.get();
      },
    }),
  };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(
            ServerStoreProvider,
            { value: store },
            createElement(OwnedUnifiedAgentSettings, {
              agent,
              state: accounts,
              notify: () => {},
              catalog,
            }),
            usageElement(store, {
              agent,
              stateDir: "loading-owner",
              limits: null,
              reload: () => {},
              visible: false,
            }),
            createElement(
              ConversationProvider,
              { value: conversationActionsFixture() },
              createElement(
                Menu,
                { opened: true, withinPortal: false },
                createElement(
                  Menu.Target,
                  null,
                  createElement("button", null, "Actions"),
                ),
                createElement(
                  Menu.Dropdown,
                  null,
                  createElement(NativeModelActions, { agent }),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    expect(host.textContent).not.toContain("xhigh");
    expect(required(h.pickers.at(-1)).disabled).toBe(true);
    expect(host.textContent).not.toContain("Context 25%");
    expect(
      required(host.querySelector<HTMLButtonElement>('[data-action="compact"]'))
        .disabled,
    ).toBe(true);
    await act(async () =>
      runInAction(() => current.set({ status: "live", data: detail.current })),
    );
    expect(host.textContent).toContain("xhigh");
    expect(required(h.pickers.at(-1)).disabled).toBe(false);
    expect(host.textContent).toContain("Context 25%");
    expect(
      required(host.querySelector<HTMLButtonElement>('[data-action="compact"]'))
        .disabled,
    ).toBe(false);
    await act(async () =>
      detail.apply({
        ...record,
        effort: "low",
        contextUsage: { tokens: 500, window: 1000 },
        nativeThreadBlock: {
          threadId: "thread",
          error: { message: "Safety block" },
        },
      }),
    );
    expect(host.textContent).toContain("low");
    expect(host.textContent).toContain("Context 50%");
    expect(
      required(host.querySelector<HTMLButtonElement>('[data-action="compact"]'))
        .disabled,
    ).toBe(true);
    expect(f.mirror.current.agents.get("lead")).toBe(agent);
    expect("effort" in agent).toBe(false);
  } finally {
    await act(async () => root.unmount());
    f.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});

it("keeps request approval disabled until its native-thread detail is known", () => {
  const f = observableFixture([record]);
  const request = approval({
    request: { kind: "exec", command: "pwd", cwd: "/", reason: null },
    params: { threadId: "thread" },
  });
  const requests = createRequestsMirror();
  requests.apply([{ collection: "request", id: request.id, value: request }]);
  const store = {
    ...f.store,
    requests: () => ({
      current: { status: "live" as const, data: requests.current },
    }),
    agentDetail: () => ({ current: { status: "loading" as const } }),
  };
  const requestOwner = new RequestAreaOwner(
    {
      api: () => requestApi,
      store,
      storage: serverLocalStorage,
      server: "local",
    },
    () => ({ scope: "owner", notify() {} }),
  );
  requestOwner.start();
  const html = render(
    f,
    createElement(
      ServerStoreProvider,
      { value: store },
      createElement(
        RequestArea,
        { owner: requestOwner },
        createElement(Requests, { all: true }),
      ),
    ),
  );
  const host = document.createElement("div");
  host.innerHTML = html;
  expect(
    required(
      [...host.querySelectorAll<HTMLButtonElement>("button")].find(
        (button) => button.textContent?.trim() === "Approve",
      ),
    ).disabled,
  ).toBe(true);
  requestOwner.dispose();
  expect(html).not.toContain("This chat stopped as a precaution");
  f.store.dispose();
});

async function mountCaller(
  f: ReturnType<typeof observableFixture>,
  view: ReactNode,
) {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const areas = conversationActionsFixture(undefined, f.store);
  ownedAreas.push(areas);
  await act(async () =>
    root.render(
      createElement(
        MantineProvider,
        { env: "test" },
        createElement(
          f.Provider,
          null,
          createElement(ServerAreasContextProvider, { value: areas }, view),
        ),
      ),
    ),
  );
  return {
    host,
    async close() {
      await act(async () => root.unmount());
      host.remove();
      f.store.dispose();
      vi.unstubAllGlobals();
    },
  };
}
function settingsCaller(
  agent: ComponentProps<typeof UnifiedAgentSettings>["agent"],
  extra: Partial<ComponentProps<typeof UnifiedAgentSettings>> = {},
) {
  return createElement(OwnedUnifiedAgentSettings, {
    agent,
    state: accounts,
    notify: () => {},
    catalog: {
      ...catalog,
      models: [
        {
          model: "gpt-6-astra",
          provider: ProviderKind.codex,
          serviceTiers: [],
          supportedReasoningEfforts: [{ reasoningEffort: "medium" }],
        },
      ],
    },
    ...extra,
  });
}
it("edits a new chat draft model, effort and role defaults without a detail subscription", async () => {
  h.catalog = {
    models: ["gpt-6-astra", "gpt-6-luna"].map((model) => ({
      model,
      provider: ProviderKind.codex,
      supportedReasoningEfforts: [
        { reasoningEffort: "high" },
        { reasoningEffort: "medium" },
      ],
    })),
    loading: false,
    error: "",
    retry: () => {},
  };
  const f = observableFixture();
  f.apply([
    {
      collection: "project",
      id: "project",
      value: { id: "project", path: "/project", accountKey: "chosen" },
    },
    {
      collection: "accounts",
      id: "current",
      value: {
        accounts: [
          {
            id: "chosen",
            provider: ProviderKind.codex,
            status: AccountStatus.ready,
            label: "Chosen",
            home: "",
            source: "fixture",
          },
        ],
        archivedAccounts: [],
        defaultAccountKey: "other",
        defaultAccountGroups: [],
        supportsDelete: true,
        supportsDisconnect: true,
      },
    },
  ]);
  h.api.get.mockClear();
  const reads = vi.spyOn(f.store, "agentDetail");
  const inherited = observableFixture();
  const selected = createServerServices({
    id: "remote",
    label: "Remote",
    origin: "https://selected.test",
  });
  Object.defineProperty(selected, "store", { value: f.store });
  const selectedAreas = conversationActionsFixture(
    undefined,
    f.store,
    selected,
  );
  ownedAreas.push(selectedAreas);
  applyFixtureCatalogs(selectedAreas);
  const model = new NewChatModelStore(
    selected,
    "project",
    "/project",
    selectedAreas,
  );
  const screen = await mountCaller(
    inherited,
    createElement(NewChatModelSettings, {
      model,
    }),
  );
  try {
    expect(required(h.pickers.at(-1)).disabled).toBe(false);
    await act(async () => required(h.pickers.at(-1)).onModel("gpt-6-luna"));
    await act(async () => required(h.pickers.at(-1)).onEffort("high"));
    expect(model.selection).toMatchObject({
      account_key: "chosen",
      model: "gpt-6-luna",
      effort: "high",
    });
    await act(async () => required(h.pickers.at(-1)).onRole("worker"));
    await act(async () => required(h.pickers.at(-1)).onModel("gpt-6-astra"));
    expect(model.selection).toMatchObject({
      worker_defaults: { model: "gpt-6-astra" },
    });
    expect(reads).not.toHaveBeenCalled();
    expect(h.api.get).not.toHaveBeenCalled();
  } finally {
    await screen.close();
    selected.close();
    f.store.dispose();
  }
});
it("queues supported running-chat settings through UnifiedAgentSettings", async () => {
  const f = observableFixture([
    { ...record, status: AgentStatus.running, nextTurnSettingsSupported: true },
  ]);
  h.api.post.mockImplementation(() => new Promise(() => {}));
  const screen = await mountCaller(
    f,
    settingsCaller(required(f.mirror.current.agents.get("lead"))),
  );
  try {
    expect(required(h.pickers.at(-1)).disabled).toBe(false);
    await act(async () => {
      void required(h.pickers.at(-1)).onEffort("medium");
    });
    expect(h.api.post).toHaveBeenCalledWith(
      "/api/conversation",
      expect.objectContaining({ next_turn: true, effort: "medium" }),
      expect.anything(),
    );
  } finally {
    await screen.close();
  }
});
it("reports an in-place settings account mismatch during a save", async () => {
  const f = observableFixture([record]);
  h.api.post.mockImplementation(() => new Promise(() => {}));
  const agent = required(f.mirror.current.agents.get("lead"));
  const screen = await mountCaller(f, settingsCaller(agent));
  try {
    await act(async () => {
      void required(h.pickers.at(-1)).onEffort("medium");
    });
    await act(async () =>
      f.apply([
        {
          collection: "agentSummary",
          id: "lead",
          value: { ...agent, accountKey: "other" },
        },
      ]),
    );
    expect(h.api.post).toHaveBeenCalled();
    expect(f.mirror.current.agents.get("lead")).toBe(agent);
    expect(screen.host.textContent).toContain(
      "The settings account changed before the change applied.",
    );
  } finally {
    await screen.close();
  }
});
it("shows numeric lead-only mode from generated detail through ChatSettingsDialog", () => {
  const f = observableFixture([
    {
      ...record,
      concurrency: 0,
      subagentConcurrencyVersion: 2,
      agentModeRevision: 0,
      agentModeSupported: true,
      agentMode: AgentMode.single,
    },
  ]);
  const agent = required(f.mirror.current.agents.get("lead"));
  const html = render(
    f,
    createElement(ChatSettingsDialog, {
      settingsOpen: true,
      accountModalOpen: false,
      setSettingsOpen: () => {},
      agent,
      lead: agent,
      accounts,
      notify: () => {},
      agentModels: catalog,
      data: Object.assign(snapshotFixture([]), {
        token: "",
        stateDir: "/fixture",
      }),
      refresh: async () => {},
      workspaceId: "workspace",
      creating: false,
      newChat: () => {},
      project: () => {},
      setClaudeLoginKey: () => {},
    }),
  );
  expect(html).toContain('data-agent-mode="single"');
  f.store.dispose();
});

it("observes peer project choices without a projects GET and preserves the grants read", async () => {
  const { ProjectPeers } =
    await import("../../projects/ProjectPeers/ProjectPeers.tsx");
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: { addEventListener() {}, removeEventListener() {} },
  });
  const f = observableFixture();
  f.apply([
    {
      collection: "project",
      id: "one",
      value: { id: "one", path: "/one", name: "First project" },
    },
  ]);
  h.api.get.mockClear();
  h.api.post.mockResolvedValue({ grants: [] });
  const form = createProjectPeersStore("one", h.api, vi.fn());
  const screen = await mountCaller(f, createElement(ProjectPeers, { form }));
  try {
    const input = screen.host.querySelector("textarea");
    if (!input) throw new Error("Missing invitation input");
    const setValue = Object.getOwnPropertyDescriptor(
      HTMLTextAreaElement.prototype,
      "value",
    )?.set;
    if (!setValue) throw new Error("Missing textarea setter");
    await act(async () => {
      setValue.call(
        input,
        JSON.stringify({
          protocol: 1,
          inviteId: "invite",
          token: "token",
          stateId: "state",
          label: "Peer",
          publicKey: "",
          expires: 1900000000,
          scope: "project",
          projectId: "peer",
        }),
      );
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    const check = [...screen.host.querySelectorAll("button")].find(
      (button) => button.textContent === "Check invitation",
    );
    if (!check) throw new Error("Missing check action");
    await act(async () => {
      check.click();
      await when(() => !form.busy);
    });
    expect(screen.host.querySelector("select")?.textContent).toContain(
      "First project",
    );
    await act(async () =>
      f.apply([
        {
          collection: "project",
          id: "one",
          value: { id: "one", path: "/one", name: "Renamed project" },
        },
      ]),
    );
    expect(screen.host.querySelector("select")?.textContent).toContain(
      "Renamed project",
    );
    expect(h.api.get).not.toHaveBeenCalled();
    expect(h.api.post).toHaveBeenCalledExactlyOnceWith("/api/federation", {
      action: "list_project_grants",
      project_id: "one",
    });
  } finally {
    await screen.close();
  }
});

it.each(["loading", "error"] as const)(
  "new-chat waits for %s state and recovers through its selected projection",
  async (status) => {
    const f = observableFixture();
    const current = observable.box<ReturnType<typeof f.store.state>["current"]>(
      status === "error"
        ? { status, error: new Error("State stream failed") }
        : { status },
      { deep: false },
    );
    vi.spyOn(f.store, "state").mockReturnValue({
      get current() {
        return current.get();
      },
    });
    const inherited = observableFixture();
    const selected = createServerServices({
      id: "remote",
      label: "Remote",
      origin: "https://recovery.test",
    });
    Object.defineProperty(selected, "store", { value: f.store });
    const selectedAreas = conversationActionsFixture(
      undefined,
      f.store,
      selected,
    );
    ownedAreas.push(selectedAreas);
    applyFixtureCatalogs(selectedAreas);
    const refresh = vi.spyOn(selected.sync, "refreshState").mockResolvedValue();
    const retryModels = vi.fn();
    h.catalog = {
      models: [
        {
          model: "gpt-6-astra",
          provider: ProviderKind.codex,
          supportedReasoningEfforts: [],
        },
      ],
      loading: false,
      error: "",
      retry: retryModels,
    };
    const model = new NewChatModelStore(
      selected,
      "project",
      "/project",
      selectedAreas,
    );
    const screen = await mountCaller(
      inherited,
      createElement(NewChatModelSettings, {
        model,
      }),
    );
    try {
      expect(model.selection).toBe(null);
      expect(refresh).not.toHaveBeenCalled();
      if (status === "error") {
        const retry = [...screen.host.querySelectorAll("button")].find(
          (button) => button.textContent === "Retry",
        );
        if (!retry) throw new Error("Missing projection retry");
        refresh.mockRejectedValueOnce(new Error("Projection retry failed"));
        await act(async () => retry.click());
        expect(
          screen.host.querySelector('[role="alert"]')?.textContent,
        ).toContain("Projection retry failed");
        await act(async () => retry.click());
        expect(refresh).toHaveBeenCalledTimes(2);
        expect(retryModels).not.toHaveBeenCalled();
      } else {
        expect(screen.host.querySelector('[role="alert"]')).toBeNull();
      }
      await act(async () => {
        f.apply([
          {
            collection: "project",
            id: "project",
            value: {
              id: "project",
              path: "/project",
              accountKey: "project-account",
            },
          },
          {
            collection: "accounts",
            id: "current",
            value: {
              accounts: ["default", "project-account"].map((id) => ({
                id,
                label: id,
                provider: ProviderKind.codex,
                status: AccountStatus.ready,
                home: "",
                source: "fixture",
              })),
              archivedAccounts: [],
              defaultAccountKey: "default",
              defaultAccountGroups: [],
              supportsDelete: true,
              supportsDisconnect: true,
            },
          },
        ]);
        runInAction(() =>
          current.set({ status: "live", data: f.mirror.current }),
        );
        applyFixtureCatalogs(selectedAreas);
      });
      expect(model.selection).toEqual(
        expect.objectContaining({
          account_key: "project-account",
          model: "gpt-6-astra",
        }),
      );
      expect(screen.host.querySelector('[role="alert"]')).toBeNull();
      expect(retryModels).not.toHaveBeenCalled();
      h.catalog = { ...required(h.catalog), error: "Model catalog failed" };
      await act(async () => required(h.pickers.at(-1)).onEffort("high"));
      const retryCatalog = [...screen.host.querySelectorAll("button")].find(
        (button) => button.textContent === "Retry",
      );
      if (!retryCatalog) throw new Error("Missing model retry");
      await act(async () => retryCatalog.click());
      expect(retryModels).toHaveBeenCalledOnce();
      expect(refresh).toHaveBeenCalledTimes(status === "error" ? 2 : 0);
    } finally {
      await screen.close();
      selected.close();
      f.store.dispose();
      vi.restoreAllMocks();
    }
  },
);

it("a successful Ask command preserves the server stream", async () => {
  const { ChatSettingsDialog: SettingsDialog } =
    await import("../../agents/ChatSettingsDialog/ChatSettingsDialog.tsx");
  const f = observableFixture([record]);
  const services = createServerServices({
    id: "settings",
    label: "Settings",
    origin: "https://settings.test",
  });
  const areas = conversationActionsFixture(() => {}, f.store, services);
  areas.navigation.selectedId = "lead";
  areas.startup.running = true;
  areas.startup.initialized = true;
  vi.spyOn(services.api, "refreshSession").mockResolvedValue({
    token: "fixture",
  });
  const refresh = vi.spyOn(services.sync, "refreshState");
  const replace = vi.spyOn(services.resources, "replaceStream");
  h.api.post.mockResolvedValue({
    ...record,
    accountKey: "default",
    permissionMode: AgentPermissionMode.default,
  });
  const screen = await mountCaller(
    f,
    createElement(SettingsDialog, {
      areas,
      accounts,
      settings: {
        chatOpen: true,
        accountModalOpen: false,
        closeChatSettings() {},
      },
      onSignIn() {},
    }),
  );
  try {
    const ask = required(
      screen.host.querySelector<HTMLInputElement>('input[value="ask"]'),
    );
    await act(async () => ask.click());
    expect(h.api.post).toHaveBeenCalledExactlyOnceWith(
      "/api/conversation",
      expect.objectContaining({ id: "lead", yolo_mode: false }),
    );
    expect(refresh).not.toHaveBeenCalled();
    expect(replace).not.toHaveBeenCalled();
  } finally {
    await screen.close();
    services.close();
    vi.restoreAllMocks();
  }
});

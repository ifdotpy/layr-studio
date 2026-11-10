import { createNet } from "../../../servers/net/net.ts";
import * as api from "../../../servers/net/api.ts";
import type { ComponentSnapshot as Snapshot } from "../ComposerAttachments/componentTestFixtures.ts";
vi.mock("../../agents/support/state.ts", async (original) => ({
  ...(await original<typeof import("../../agents/support/state.ts")>()),
  useAgentDetail: <T>(agent: T) => agent,
}));
import {
  AccountStatus as GeneratedApiAccountStatus,
  ProviderKind as GeneratedApiProviderKind,
} from "../../../generated/api.ts";
import type { ComponentHookCell } from "../ComposerAttachments/componentTestFixtures.ts";
import { required } from "../ComposerAttachments/componentTestFixtures.ts";
import type { EffectCallback, PropsWithChildren } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { createElement, isValidElement, type ComponentProps } from "react";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { Agent, Message } from "../../../types.ts";
import { componentSnapshot as snapshotFixture } from "../ComposerAttachments/componentTestFixtures.ts";
import type { Account } from "../../accounts/support/Accounts.tsx";
import { createServerStore } from "../../../store/server.ts";
import { createSubscriptions } from "../../../sync/subscriptions/module.ts";
const actionStore = () =>
  createServerStore(
    createSubscriptions({
      watchResourceChanges: () => () => {},
      watchResourceConnection: () => () => {},
    }),
  );
let reads = actionStore();

const h = vi.hoisted(() => ({
  cost: undefined as
    | import("../../../generated/api.ts").components["schemas"]["SessionCostResponse"]
    | undefined,
  cursor: 0,
  cells: [] as ComponentHookCell[],
  effects: [] as Array<() => void>,
  get: vi.fn(),
  post: vi.fn(),
  subscribe: vi.fn(
    (_callback: (snapshot: Snapshot | null) => void) => () => {},
  ),
}));
vi.mock("react", async (original) => ({
  ...(await original<typeof import("react")>()),
  useState: (initial: unknown) => {
    const index = h.cursor++;
    h.cells[index] ??= {
      value: typeof initial === "function" ? initial() : initial,
    };
    return [
      required(h.cells[index]).value,
      (value: unknown) => {
        required(h.cells[index]).value =
          typeof value === "function"
            ? value(required(h.cells[index]).value)
            : value;
      },
    ];
  },
  useRef: (initial: unknown) => {
    const index = h.cursor++;
    h.cells[index] ??= { current: initial };
    return h.cells[index];
  },
  useMemo: (compute: () => unknown) => compute(),
  useCallback: (callback: unknown) => callback,
  useEffect: (effect: EffectCallback, deps: unknown[]) => {
    const index = h.cursor++;
    const previous = h.cells[index];
    if (
      !previous ||
      deps.some((value, i) => !Object.is(value, previous.deps![i]))
    ) {
      previous?.cleanup?.();
      h.cells[index] = { deps };
      h.effects.push(() => {
        required(h.cells[index]).cleanup =
          effect() as ComponentHookCell["cleanup"];
      });
    }
  },
}));
const detailStore = {
  get reads() {
    return reads.reads;
  },
  agentDetail: () => ({ current: { status: "loading" } }),
  sessionCost: () => ({
    current: h.cost ? { status: "live", data: h.cost } : { status: "absent" },
  }),
};
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerNet: () =>
      createNet({ ...api, get: h.get, post: h.post }, () => reads),
    useServerStore: () => detailStore,
    useServerApi: () => ({
      get: h.get,
      post: h.post,
      saved: () => null,
      getStreamSession: () => undefined,
    }),
    useServerStorage: () => ({}),
    useServerSync: () => ({ subscribeStateProjection: h.subscribe }),
  }),
);
vi.mock("../../agents/support/WorkerModelPicker.tsx", () => ({
  useWorkerModels: () => ({ models: [], loading: false }),
}));
vi.mock("../ClaudeFileBackups/ClaudeFileBackups.tsx", () => ({
  default: "file-backups",
}));
vi.mock("../../accounts/AccountSignInNotice/AccountSignInNotice.tsx", () => ({
  default: "sign-in",
}));
vi.mock(
  "../../projects/ProjectDirectoryPicker/ProjectDirectoryPicker.tsx",
  () => ({ default: "directory-picker" }),
);

import { useTurnErrors } from "../transcript/TurnHistory/useTurnErrors.ts";
import MessageInfo from "../transcript/MessageInfo/MessageInfo.tsx";
vi.mock("mobx-react-lite", async (original) => ({
  ...(await original<typeof import("mobx-react-lite")>()),
  observer: <T>(component: T) => component,
}));

function messageInfo(props: ComponentProps<typeof MessageInfo>) {
  const node = MessageInfo(props);
  if (!isValidElement<{ onClick(): void; onChange(open: boolean): void }>(node))
    throw new Error("Expected the message info trigger or popover");
  return node;
}
import { ClaudeSettingsView as ClaudeSettings } from "../../settings/ClaudeSettingsView/ClaudeSettingsView.tsx";
import ClaudeRepairNotice from "../ClaudeRepairNotice/ClaudeRepairNotice.tsx";
import ProjectLocationsSettings from "../../settings/ProjectLocationsSettings/ProjectLocationsSettings.tsx";
import { NativeNotice } from "../../shell/NativeNotice/NativeNotice.tsx";

function render<T>(read: () => T) {
  h.cursor = 0;
  const value = read();
  for (const effect of h.effects.splice(0)) effect();
  return value;
}
async function settle() {
  for (let i = 0; i < 8; i++) await Promise.resolve();
}
function find(
  node: ReactNode,
  predicate: (
    node: ReactElement<PropsWithChildren<Record<string, unknown>>>,
  ) => boolean,
): ReactElement<PropsWithChildren<Record<string, unknown>>> | undefined {
  if (Array.isArray(node)) {
    for (const child of node) {
      const match = find(child, predicate);
      if (match) return match;
    }
  } else if (node && typeof node === "object" && "props" in node) {
    const element = node as ReactElement<
      PropsWithChildren<Record<string, unknown>>
    >;
    return predicate(element)
      ? element
      : find(element.props.children, predicate);
  }
  return undefined;
}
afterEach(() => {
  reads.dispose();
  reads = actionStore();
  for (const cell of h.cells) cell?.cleanup?.();
  vi.unstubAllGlobals();
  h.cells = [];
  h.effects = [];
  h.get.mockReset();
  h.post.mockReset();
  h.subscribe.mockClear();
});

const agent: Agent = {
  id: "detail-agent",
  threadId: "thread",
  accountKey: "claude",
  epoch: 1,
};

it("new failed and uncertain turns use inline errors without analytics reads", async () => {
  h.get.mockResolvedValue({ turns: [] });
  render(() => useTurnErrors([], agent, "live-turns"));
  await settle();
  for (const turnStatus of ["failed", "uncertain"] as const) {
    const items: Message[] = [
      {
        id: turnStatus,
        role: "assistant",
        text: "answer",
        turnId: turnStatus,
        turnStatus,
      },
    ];
    const result = render(() => useTurnErrors(items, agent, "live-turns"));
    await settle();
    expect(result.loading.length).toBe(0);
    expect(h.get).not.toHaveBeenCalled();
    const inline = render(() =>
      useTurnErrors(
        [{ ...required(items[0]), turnError: "Streamed failure" }],
        agent,
        "live-turns",
      ),
    );
    expect(
      inline.items.some(
        (item) =>
          item.nativeError === "Streamed failure" ||
          item.turnError === "Streamed failure",
      ),
    ).toBe(true);
    if (turnStatus === "failed") {
      const markup = renderToStaticMarkup(
        createElement(NativeNotice, { item: inline.items.at(-1)! }),
      );
      expect(markup).toContain("Streamed failure");
      expect(h.get).not.toHaveBeenCalled();
    }
  }
});

it("historical error first load and an explicit retry still read once each", async () => {
  h.get
    .mockRejectedValueOnce(new Error("unavailable"))
    .mockResolvedValue({ turns: [] });
  const items: Message[] = [
    {
      id: "past",
      role: "assistant",
      text: "",
      turnId: "past",
      turnStatus: "failed",
    },
  ];
  const read = () => useTurnErrors(items, agent, "past-turns");
  render(read);
  await settle();
  const result = render(read);
  expect(result.loading.length).toBe(0);
  expect(result.failed.includes("past")).toBe(true);
  result.retry("past");
  render(read);
  await settle();
  expect(h.get).toHaveBeenCalledTimes(2);
});

it("opening a different thread loads its historical errors once", async () => {
  h.get.mockResolvedValue({ turns: [] });
  const items: Message[] = [
    {
      id: "old",
      role: "assistant",
      text: "",
      turnId: "old",
      turnStatus: "failed",
    },
  ];
  render(() => useTurnErrors(items, agent, "thread-switch"));
  await settle();
  const nextAgent = { ...agent, threadId: "next-thread" };
  render(() => useTurnErrors(items, nextAgent, "thread-switch"));
  await settle();
  expect(h.get).toHaveBeenCalledTimes(2);
  expect(required(required(h.get.mock.calls[1])[1]).query.thread).toBe(
    "next-thread",
  );
  render(() =>
    useTurnErrors(
      [...items, { ...required(items[0]), id: "live", turnId: "live" }],
      nextAgent,
      "thread-switch",
    ),
  );
  await settle();
  expect(h.get).toHaveBeenCalledTimes(2);
});

it("message info reads on open and reopen but not streamed turn metadata changes", async () => {
  h.get.mockResolvedValue({ model: "queried-model" });
  let message: Message = {
    id: "item",
    role: "assistant",
    text: "answer",
    streaming: true,
  };
  const read = () =>
    messageInfo({
      message,
      agentId: agent.id,
      stateDir: "fixture",
      onCopy: vi.fn(),
    });
  let view = render(read);
  expect(h.get).not.toHaveBeenCalled();
  // The closed message info is its plain trigger; the popover mounts on open.
  view.props.onClick();
  render(read);
  await settle();
  message = {
    ...message,
    turnId: "turn",
    threadId: "thread",
    streaming: false,
    turnStatus: "completed",
  };
  view = render(read);
  await settle();
  expect(h.get).toHaveBeenCalledTimes(1);
  expect(
    find(
      view,
      (node) => node.type === "dd" && node.props.children === "queried-model",
    ),
  ).toBeDefined();
  view.props.onChange(false);
  view = render(read);
  view.props.onChange(true);
  render(read);
  await settle();
  expect(h.get).toHaveBeenCalledTimes(2);
});

it("message info uses streamed cost and leaves missing cost unavailable", async () => {
  h.get.mockResolvedValue({});
  const read = () =>
    messageInfo({
      message: { id: "item", role: "assistant", text: "answer" },
      agentId: agent.id,
      stateDir: "fixture",
      rootId: "root",
      onCopy: vi.fn(),
    });
  let view = render(read);
  view.props.onClick();
  render(read);
  const notify = (snapshot: Snapshot) => {
    h.cost = snapshot.runtime.sessionCostsByAgent?.root;
  };
  const cost = {
    rootId: "root",
    pricingState: "ready",
    totalUSD: null,
    estimated: true,
    refreshing: false,
    cacheAgeSeconds: 0,
    pricedSamples: 0,
    unknownModels: [],
    breakdown: { models: {}, providers: {} },
  } satisfies import("../../../generated/api.ts").components["schemas"]["SessionCostResponse"];
  notify(
    Object.assign(
      snapshotFixture([], { sessionCostsByAgent: { root: cost } }),
      { token: "", stateDir: "/fixture" },
    ),
  );
  view = render(read);
  expect(
    find(
      view,
      (node) =>
        node.type === "dt" && node.props.children === "Session estimate (USD)",
    ),
  ).toBeUndefined();
  notify(
    Object.assign(
      snapshotFixture([], {
        sessionCostsByAgent: { root: { ...cost, totalUSD: 1 } },
      }),
      { token: "", stateDir: "/fixture" },
    ),
  );
  view = render(read);
  expect(
    find(
      view,
      (node) =>
        node.type === "dt" && node.props.children === "Session estimate (USD)",
    ),
  ).toBeDefined();
  notify(Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }));
  view = render(read);
  expect(
    find(
      view,
      (node) =>
        node.type === "dt" && node.props.children === "Session estimate (USD)",
    ),
  ).toBeUndefined();
});

it("Claude account status and key updates do not reload settings; explicit retry does", async () => {
  h.post.mockRejectedValue(new Error("Sign in with claude auth login first"));
  let currentAgent = agent;
  let account: Account = {
    id: "claude",
    home: "/fixture",
    source: "fixture",
    status: GeneratedApiAccountStatus.signedOut,
    provider: GeneratedApiProviderKind.claude,
    label: "Claude",
  };
  const read = () => ClaudeSettings({ agent: currentAgent, account });
  render(read);
  await settle();
  account = { ...account, status: GeneratedApiAccountStatus.ready } as Account;
  currentAgent = { ...agent, accountKey: "other" };
  const view = render(read);
  await settle();
  expect(h.post).toHaveBeenCalledTimes(1);
  const retry = find(
    view,
    (node) => node.props.children === "Retry Claude settings",
  );
  expect(retry).toBeDefined();
  h.post.mockResolvedValue({
    settings: { permissionMode: "acceptEdits" },
    turns: [],
    tasks: [],
  });
  (retry!.props.onClick as () => void)();
  render(read);
  await settle();
  expect(h.post).toHaveBeenCalledTimes(2);
  expect(render(read).props["aria-busy"]).toBe(false);
  expect(
    find(render(read), (node) => node.props.label === "Permission mode")!.props
      .value,
  ).toBe("acceptEdits");
  expect(
    find(
      render(read),
      (node) => node.props.children === "Retry Claude settings",
    ),
  ).toBeUndefined();
});

it("repair scope changes hide old evidence without reads; refresh uses the current agent", async () => {
  h.get.mockResolvedValue({ status: "none" });
  let currentAgent = { ...agent, error: "CLAUDE_SESSION_REPAIR_REQUIRED" };
  const read = () => ClaudeRepairNotice({ agent: currentAgent });
  render(read);
  await settle();
  for (const patch of [
    { epoch: 2 },
    { accountKey: "other" },
    { threadId: "new-thread" },
  ]) {
    currentAgent = { ...currentAgent, ...patch };
    render(read);
    await settle();
    expect(h.get).toHaveBeenCalledTimes(1);
  }
  const refresh = find(
    render(read),
    (node) => node.props.children === "Refresh saved report",
  );
  (refresh!.props.onClick as () => void)();
  render(read);
  await settle();
  expect(h.get).toHaveBeenCalledTimes(2);
});

it("location menu reads once and location changes do not refetch folder info", async () => {
  vi.stubGlobal("window", { addEventListener() {}, removeEventListener() {} });
  h.get.mockResolvedValue({ gitHead: "initial-head" });
  const location = { serverId: "local", path: "/one", projectId: "project" };
  let project = {
    id: "project",
    name: "Project",
    path: "/one",
    locations: [location],
  };
  const read = () => {
    const view = ProjectLocationsSettings({ project });
    if (!isValidElement(view)) throw new Error("Expected location settings");
    return view;
  };
  render(read);
  await settle();
  expect(h.get).not.toHaveBeenCalled();
  const menu = find(
    render(read),
    (node) =>
      node.props.opened === false && typeof node.props.onChange === "function",
  );
  const open = menu?.props.onChange;
  if (typeof open !== "function") throw new Error("Missing location menu");
  open(true);
  await settle();
  expect(h.get).toHaveBeenCalledTimes(1);
  for (const locations of [
    [{ ...location }],
    [{ ...location, path: "/two" }],
    [location, { ...location, serverId: "remote" }],
  ]) {
    project = { ...project, locations };
    const view = render(read);
    await settle();
    expect(h.get).toHaveBeenCalledTimes(1);
    const row = find(
      view,
      (node) => node.props.className === "project-location-row",
    );
    const label = row?.props.label;
    if (!isValidElement(label)) throw new Error("Missing location label");
    if (locations[0]?.path === "/two")
      expect(renderToStaticMarkup(label)).not.toContain("initial-head");
  }
});

it("overlapping location HEAD actions keep their requested paths isolated", async () => {
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    callback(0);
    return 0;
  });
  vi.stubGlobal("window", { addEventListener() {}, removeEventListener() {} });
  const one = Promise.withResolvers<{ gitHead: string }>(),
    two = Promise.withResolvers<{ gitHead: string }>();
  h.get.mockReturnValueOnce(one.promise).mockReturnValueOnce(two.promise);
  let project = {
    id: "project",
    name: "Project",
    path: "/one",
    locations: [{ projectId: "project", serverId: "local", path: "/one" }],
  };
  const read = () => {
    const view = ProjectLocationsSettings({ project });
    if (!isValidElement(view)) throw new Error("Missing settings");
    return view;
  };
  const menu = () => {
    const node = find(
      render(read),
      (node) =>
        typeof node.props.onChange === "function" && "opened" in node.props,
    );
    const change = node?.props.onChange;
    if (typeof change !== "function") throw new Error("Missing menu");
    return change;
  };
  menu()(true);
  await settle();
  menu()(false);
  project = {
    ...project,
    locations: [{ projectId: "project", serverId: "local", path: "/two" }],
  };
  menu()(true);
  await settle();
  expect(h.get).toHaveBeenCalledTimes(2);
  expect(h.get).toHaveBeenNthCalledWith(
    2,
    "/api/project-locations",
    expect.objectContaining({
      query: expect.objectContaining({ path: "/two" }),
    }),
  );
  two.resolve({ gitHead: "head-two" });
  await settle();
  one.resolve({ gitHead: "head-one" });
  await settle();
  const label = find(
    render(read),
    (node) => node.props.className === "project-location-row",
  )?.props.label;
  if (!isValidElement(label)) throw new Error("Missing location label");
  expect(renderToStaticMarkup(label)).toContain("head-two");
  expect(renderToStaticMarkup(label)).not.toContain("head-one");
});

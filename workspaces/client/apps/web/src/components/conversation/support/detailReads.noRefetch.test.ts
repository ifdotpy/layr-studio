import { ClaudeRepairReportStore } from "../ClaudeRepairNotice/ClaudeRepairReportStore.ts";
import { autorun, Reaction } from "mobx";
import { ClaudeSettingsStore } from "../../settings/ClaudeSettingsView/ClaudeSettingsStore.ts";
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
import { createElement } from "react";
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
  useMemo: (compute: () => unknown, deps: unknown[]) => {
    const index = h.cursor++;
    const previous = h.cells[index];
    if (
      !previous ||
      deps.some((value, i) => !Object.is(value, previous.deps?.[i]))
    )
      h.cells[index] = { deps, value: compute() };
    return required(h.cells[index]).value;
  },
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
  sessionCost: (): ReturnType<MessageInfoSource["sessionCost"]> => ({
    current: h.cost ? { status: "live", data: h.cost } : { status: "absent" },
  }),
};
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerServices: () => undefined,
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

import { TurnErrorStore } from "./useTurnErrors.ts";
import {
  MessageInfoStore,
  type MessageInfoSource,
} from "../transcript/MessageInfo/MessageInfoStore.ts";
vi.mock("mobx-react-lite", async (original) => ({
  ...(await original<typeof import("mobx-react-lite")>()),
  observer: <T>(component: T) => component,
}));

import { ClaudeSettingsView as ClaudeSettings } from "../../settings/ClaudeSettingsView/ClaudeSettingsView.tsx";
import ClaudeRepairNotice from "../ClaudeRepairNotice/ClaudeRepairNotice.tsx";
import { NativeNotice } from "../../shell/NativeNotice/NativeNotice.tsx";

let renderReaction = new Reaction("detail reads harness", () => {});
function render<T>(read: () => T) {
  h.cursor = 0;
  const value = read();
  for (const effect of h.effects.splice(0)) effect();
  return value;
}
function renderObserved<T>(read: () => T) {
  let value!: T;
  renderReaction.track(() => {
    value = render(read);
  });
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
  renderReaction.dispose();
  renderReaction = new Reaction("detail reads harness", () => {});
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
  const errors = new TurnErrorStore([]);
  const net = createNet({ ...api, get: h.get, post: h.post }, () => reads);
  renderObserved(() => errors.read([], agent, reads, net));
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
    const result = renderObserved(() => errors.read(items, agent, reads, net));
    await settle();
    expect(result.loading.length).toBe(0);
    expect(h.get).not.toHaveBeenCalled();
    const inline = renderObserved(() =>
      errors.read(
        [{ ...required(items[0]), turnError: "Streamed failure" }],
        agent,
        reads,
        net,
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
  const errors = new TurnErrorStore(items);
  const net = createNet({ ...api, get: h.get, post: h.post }, () => reads);
  const read = () => errors.read(items, agent, reads, net);
  renderObserved(read);
  await settle();
  const result = renderObserved(read);
  expect(result.loading.length).toBe(0);
  expect(result.failed.includes("past")).toBe(true);
  result.retry("past");
  renderObserved(read);
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
  let errors = new TurnErrorStore(items);
  const net = createNet({ ...api, get: h.get, post: h.post }, () => reads);
  renderObserved(() => errors.read(items, agent, reads, net));
  await settle();
  const nextAgent = { ...agent, threadId: "next-thread" };
  errors = new TurnErrorStore(items);
  renderObserved(() => errors.read(items, nextAgent, reads, net));
  await settle();
  expect(h.get).toHaveBeenCalledTimes(2);
  expect(required(required(h.get.mock.calls[1])[1]).query.thread).toBe(
    "next-thread",
  );
  renderObserved(() =>
    errors.read(
      [...items, { ...required(items[0]), id: "live", turnId: "live" }],
      nextAgent,
      reads,
      net,
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
  const model = new MessageInfoStore();
  const net = createNet({ ...api, get: h.get, post: h.post }, () => reads);
  const read = () =>
    model.view(message, agent.id, undefined, undefined, detailStore);
  expect(h.get).not.toHaveBeenCalled();
  model.changeOpen(true, read().query, detailStore, net);
  renderObserved(read);
  await settle();
  message = {
    ...message,
    turnId: "turn",
    threadId: "thread",
    streaming: false,
    turnStatus: "completed",
  };
  const view = renderObserved(read);
  await settle();
  expect(h.get).toHaveBeenCalledTimes(1);
  expect(view.rows).toContainEqual(["Model", "queried-model"]);
  model.close();
  model.changeOpen(true, read().query, detailStore, net);
  renderObserved(read);
  await settle();
  expect(h.get).toHaveBeenCalledTimes(2);
});

it("message info uses streamed cost and leaves missing cost unavailable", async () => {
  h.get.mockResolvedValue({});
  const model = new MessageInfoStore();
  const net = createNet({ ...api, get: h.get, post: h.post }, () => reads);
  const message: Message = { id: "item", role: "assistant", text: "answer" };
  const read = () =>
    model.view(message, agent.id, "root", undefined, detailStore);
  model.changeOpen(true, read().query, detailStore, net);
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
  expect(
    read().rows.some(([label]) => label === "Session estimate (USD)"),
  ).toBe(false);
  notify(
    Object.assign(
      snapshotFixture([], {
        sessionCostsByAgent: { root: { ...cost, totalUSD: 1 } },
      }),
      { token: "", stateDir: "/fixture" },
    ),
  );
  expect(
    read().rows.some(([label]) => label === "Session estimate (USD)"),
  ).toBe(true);
  notify(Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }));
  expect(
    read().rows.some(([label]) => label === "Session estimate (USD)"),
  ).toBe(false);
});

it("Claude account status and key updates do not reload settings; explicit retry does", async (ctx) => {
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
  const model = new ClaudeSettingsStore(
    { ...api, post: h.post },
    () => currentAgent,
  );
  const stop = autorun(() => model.state);
  ctx.onTestFinished(() => {
    stop();
    model.dispose();
  });
  const read = () => ClaudeSettings({ agent: currentAgent, account, model });
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

it("repair scope changes hide old evidence without reads; refresh uses the current agent", async (ctx) => {
  h.get.mockResolvedValue({ status: "none" });
  let currentAgent = { ...agent, error: "CLAUDE_SESSION_REPAIR_REQUIRED" };
  const report = new ClaudeRepairReportStore(
    { ...api, get: h.get },
    () => currentAgent,
  );
  const stop = autorun(() => report.current);
  ctx.onTestFinished(() => {
    stop();
    report.dispose();
  });
  const read = () => ClaudeRepairNotice({ agent: currentAgent, store: report });
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

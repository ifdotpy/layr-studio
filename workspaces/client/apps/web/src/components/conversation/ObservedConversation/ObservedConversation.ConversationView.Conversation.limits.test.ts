import { observer } from "mobx-react-lite";
import { renderMessageRow } from "../ConversationTranscript/ConversationTranscript.tsx";
import { createNet } from "../../../servers/net/net.ts";
// @vitest-environment happy-dom
import type { Agent } from "../../../types.ts";
import { ServerAreasContextProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  conversationActionsFixture,
  setStartupFixture,
} from "../../shell/integration-support/observableFixture.tsx";
vi.mock("../support/requestView.ts", () => ({
  createRequestsView: () => ({ requests: [] }),
}));
import { required } from "../ComposerAttachments/componentTestFixtures.ts";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import PromptInput from "../prompt-composer/PromptInput/PromptInput.tsx";
vi.mock("../../agents/support/state.ts", async (original) => ({
  ...(await original<typeof import("../../agents/support/state.ts")>()),
  useAgentCatalog: () => ({ agents: [] }),
}));
import {
  AgentSource as GeneratedApiAgentSource,
  AgentStatus as GeneratedApiAgentStatus,
} from "../../../generated/api.ts";
import { afterEach, expect, it, vi } from "vitest";
import {
  isValidElement,
  type ComponentProps,
  type ReactElement,
  type ReactNode,
} from "react";

const h = vi.hoisted(() => ({
  live: false,
  areas: undefined as
    | import("../../../app/ServerAreasProvider.tsx").ServerAreas
    | undefined,
  store: undefined as
    | ReturnType<
        typeof import("../../shell/integration-support/observableFixture.tsx").observableFixture
      >["store"]
    | undefined,
  api: undefined as
    | import("../../../servers/serverServices.ts").ServerServices["api"]
    | undefined,
  effects: [] as Array<() => unknown>,
  read: vi.fn<typeof import("../../../servers/net/api.ts").get>(),
}));
vi.mock("../../../usePendingCommands.ts", () => ({
  usePendingCommands: () => [],
}));
vi.mock("react", async (original) => {
  const actual = await original<typeof import("react")>();
  return {
    ...actual,
    useContext: <T>(context: import("react").Context<T>) =>
      h.live ? actual.useContext(context) : { areas: h.areas },
    useState: (initial: unknown) =>
      h.live
        ? actual.useState(initial)
        : [typeof initial === "function" ? initial() : initial, vi.fn()],
    useRef: (current: unknown) =>
      h.live ? actual.useRef(current) : { current },
    useMemo: (compute: () => unknown, deps: unknown[]) =>
      h.live ? actual.useMemo(compute, deps) : compute(),
    useCallback: (callback: () => unknown, deps: unknown[]) =>
      h.live ? actual.useCallback(callback, deps) : callback,
    useLayoutEffect: (effect: () => void | (() => void), deps: unknown[]) => {
      if (h.live) actual.useLayoutEffect(effect, deps);
    },
    useEffect: (effect: () => void | (() => void), deps: unknown[]) => {
      if (h.live) actual.useEffect(effect, deps);
      else h.effects.push(effect);
    },
  };
});
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerStore: () =>
      h.store ?? {
        state: () => ({
          current: {
            status: "live",
            data: {
              agents: new Map(),
              workspaces: new Map([["workspace", { stateDir: "fixture" }]]),
            },
          },
        }),
      },
    useServerApi: () => h.api ?? { get: h.read, post: vi.fn() },
    useServerNet: () => {
      const api = h.api,
        store = h.store;
      if (!api || !store)
        throw new Error("The live read fixture requires its server owner.");
      return createNet(api, () => store);
    },
    useServerSync: () => ({ subscribeStateProjection: () => () => {} }),
    useServerOutbox: () => ({}),
    useServerUploads: () => ({}),
  }),
);
vi.mock("../../../conversation/liveText.ts", async (original) => ({
  ...(await original<typeof import("../../../conversation/liveText.ts")>()),
  useLiveText: () => new Map(),
}));
vi.mock("@mantine/hooks", async (original) => ({
  ...(await original<typeof import("@mantine/hooks")>()),
  useMediaQuery: () => false,
}));
vi.mock("../support/useQuestionCards.ts", () => ({
  createQuestionCards: () => () => undefined,
}));
vi.mock("../TokenRate/TokenRate.tsx", () => ({ default: () => null }));
vi.mock("../support/usePromptRecall.ts", () => ({
  usePromptRecall: () => ({}),
}));
vi.mock("../support/useChatReadState.ts", () => ({
  useVisibleChatResult: () => {},
}));
vi.mock("../../agents/support/WorkerModelPicker.tsx", async (original) => ({
  ...(await original<
    typeof import("../../agents/support/WorkerModelPicker.tsx")
  >()),
  useWorkerModels: () => ({ models: [] }),
}));
vi.mock("../../../wasm/client.ts", () => ({}));
import { renderConversationViewFixture as Conversation } from "../ConversationView/ConversationView.fixture.tsx";
import { ObservedNativeError as NativeError } from "../ObservedNativeError/ObservedNativeError.tsx";
import PromptComposer from "../prompt-composer/PromptComposer/PromptComposer.tsx";
import RealtimeVoice from "../RealtimeVoice/RealtimeVoice.tsx";

type Props = { agent?: Agent; detail?: Agent };
function render(
  agent: Props["agent"],
  reloadLimits: () => void,
  detail: Props["detail"] = agent,
) {
  const models = observableFixture(agent ? [agent] : []);
  const actions = conversationActionsFixture(() => {}, models.store);
  actions.navigation.selectedId = agent?.id || null;
  setStartupFixture(actions, { stateDir: "fixture" });
  actions.layout.geometry.viewport.width = 1280;
  if (agent && detail) models.details.get(agent.id)?.apply(detail);
  else if (agent) models.details.delete(agent.id);
  h.read.mockImplementation(async () => {
    reloadLimits();
    return undefined;
  });
  vi.spyOn(actions.scope.services.api, "get").mockImplementation(h.read);
  h.areas = actions;
  const element = Conversation();
  const cleanups = h.effects.splice(0).map((effect) => effect());
  for (const cleanup of cleanups) if (typeof cleanup === "function") cleanup();
  vi.unstubAllGlobals();
  models.store.dispose();
  actions.scope.services.close();
  return element;
}
afterEach(() => {
  h.effects = [];
  h.live = false;
  h.store = undefined;
  h.api = undefined;
  vi.unstubAllGlobals();
});

function elements(node: ReactNode): ReactElement<{ children?: ReactNode }>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<{ children?: ReactNode }>(node)) return [];
  return [node, ...elements(node.props.children)];
}

it.each([undefined, null, "future-provider", "claude", "codex"])(
  "the conversation only mounts realtime voice for an explicit Codex provider (%s)",
  (provider) => {
    const agent: NonNullable<Props["agent"]> = {
      id: "lead",
      source: GeneratedApiAgentSource.managed,
      isLead: true,
      status: GeneratedApiAgentStatus.idle,
      // @ts-expect-error Future identities must not enable the voice capability.
      provider,
    };
    const tree = render(agent, () => {});
    const composer = elements(tree).find(
      (node) => node.type === PromptComposer,
    );
    expect(
      isValidElement<ComponentProps<typeof PromptComposer>>(composer),
    ).toBe(true);
    if (!isValidElement<ComponentProps<typeof PromptComposer>>(composer))
      return;
    const tools = elements(composer.props.children(""));
    expect(tools.some((node) => node.type === RealtimeVoice)).toBe(
      provider === "codex",
    );
  },
);

it.each(["usageLimitExceeded", "rateLimitExceeded"])(
  "does not read limits after the streamed agent error changes to %s",
  (kind) => {
    const read = vi.fn();
    const reloadLimits = () => read("/api/limits");
    render({ id: "lead", turnId: "turn" }, reloadLimits);
    render(
      {
        id: "lead",
        turnId: "turn",
        error: { codexErrorInfo: kind },
      },
      reloadLimits,
    );
    expect(read).not.toHaveBeenCalled();
  },
);

it("retains the explicit limits refresh action", () => {
  const reloadLimits = vi.fn();
  const element = render(
    {
      id: "lead",
      error: { codexErrorInfo: "usageLimitExceeded" },
    },
    reloadLimits,
  );
  const error = elements(element).find((child) => child.type === NativeError);
  if (!isValidElement<ComponentProps<typeof NativeError>>(error))
    throw new Error("Expected native error");
  reloadLimits.mockClear();
  required(error.props.refreshLimits)();
  expect(reloadLimits).toHaveBeenCalledOnce();
});

it("enables the composer from generated detail without copying it onto the summary", () => {
  const fixture = observableFixture([
    { id: "lead", source: GeneratedApiAgentSource.managed, canSend: true },
  ]);
  const summary = required(fixture.mirror.current.agents.get("lead"));
  const detail = required(fixture.details.get("lead")).current;
  const tree = render(summary, () => {}, detail);
  const composer = elements(tree).find((node) => node.type === PromptComposer);
  expect(isValidElement<ComponentProps<typeof PromptComposer>>(composer)).toBe(
    true,
  );
  if (!isValidElement<ComponentProps<typeof PromptComposer>>(composer))
    throw new Error("Expected composer");
  const input = elements(composer.props.children("hello")).find(
    (node) => node.type === PromptInput,
  );
  expect(isValidElement<ComponentProps<typeof PromptInput>>(input)).toBe(true);
  if (!isValidElement<ComponentProps<typeof PromptInput>>(input))
    throw new Error("Expected prompt input");
  expect(input.props.canSend).toBe(true);
  fixture.store.dispose();
});

import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import Usage from "../Usage/Usage.tsx";
import MessageActions from "../transcript/MessageActions/MessageActions.tsx";
import MessageInfo from "../transcript/MessageInfo/MessageInfo.tsx";
import { TranscriptProvider } from "../support/navigation.ts";
import { projectedLimits } from "../../../usage/accountLimits.fixture.ts";
import { serializeStateCollections } from "../../../generated/observableResources.ts";
import {
  AccountStatus,
  EligibilityState,
  Overage,
  ProviderKind,
} from "../../../generated/api.ts";
import type { Message } from "../../../types.ts";

it("keeps chat A metadata and recovery while the real Usage dropdown browses B", async () => {
  h.live = true;
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  const models = observableFixture([
    {
      id: "lead",
      accountKey: "account-A",
      source: GeneratedApiAgentSource.managed,
      provider: ProviderKind.codex,
      isLead: true,
      status: GeneratedApiAgentStatus.idle,
      canSend: true,
      error: { codexErrorInfo: "usageLimitExceeded" },
    },
  ]);
  const a = projectedLimits("account-A", 100),
    b = projectedLimits("account-B", 200);
  a.observation.data = {
    accountId: "native-A",
    rateLimits: { planType: "plus" },
  };
  b.observation.data = {
    accountId: "native-B",
    rateLimits: { planType: "pro" },
  };
  for (const [key, view, title] of [
    ["account-A", a, "Recovery A"],
    ["account-B", b, "Recovery B"],
  ] as const) {
    view.projection.agents.lead = {
      eligibility: {
        accountKey: key,
        agentId: "lead",
        buckets: [],
        model: "fixture",
        overage: Overage.unavailable,
        provider: ProviderKind.codex,
        state: EligibilityState.blocked,
        validUntil: null,
      },
      recovery: {
        title,
        message: title,
        timing: { kind: "unknown" },
        action: { kind: "none" },
      },
    };
  }
  const account = {
    id: "account-A",
    accountId: "native-A",
    label: "Account A",
    provider: ProviderKind.codex,
    status: AccountStatus.ready,
    home: "",
    source: "fixture",
  };
  models.apply([
    ...serializeStateCollections(models.mirror.current),
    {
      collection: "accounts",
      id: "current",
      value: {
        accounts: [
          account,
          {
            ...account,
            id: "account-B",
            accountId: "native-B",
            label: "Account B",
          },
        ],
        archivedAccounts: [],
        defaultAccountKey: "account-A",
        defaultAccountGroups: [],
        supportsDelete: true,
        supportsDisconnect: true,
      },
    },
    { collection: "rateLimit", id: "account-A", value: a },
    { collection: "rateLimit", id: "account-B", value: b },
  ]);
  const areas = conversationActionsFixture(() => {}, models.store);
  areas.navigation.selectedId = "lead";
  setStartupFixture(areas, { stateDir: "fixture" });
  areas.layout.geometry.viewport.width = 1280;
  h.store = models.store;
  h.api = areas.scope.services.api;
  const reads = vi.spyOn(h.api, "get");
  const message: Message = {
    id: "answer",
    role: "assistant",
    text: "Answer",
    accountKey: "account-A",
    model: "fixture",
  };
  let native: ComponentProps<typeof NativeError> | undefined;
  let usageProps: ComponentProps<typeof Usage> | undefined;
  const Caller = observer(function Caller() {
    const tree = Conversation();
    const error = elements(tree).find((node) => node.type === NativeError);
    if (!isValidElement<ComponentProps<typeof NativeError>>(error))
      throw new Error("Expected native error");
    native = error.props;
    const usage = elements(tree).find((node) => node.type === Usage);
    if (!isValidElement<ComponentProps<typeof Usage>>(usage))
      throw new Error("Expected Usage");
    usageProps = usage.props;
    const transcript = elements(tree).find(
      (node) => node.type === TranscriptProvider,
    );
    if (!isValidElement<ComponentProps<typeof TranscriptProvider>>(transcript))
      throw new Error("Expected transcript provider");
    const row = required(transcript.props.value).renderMessage(message);
    if (!isValidElement<{ item: Message }>(row))
      throw new Error("Expected message row");
    const article = renderMessageRow({ item: message });
    const actions = elements(article).find(
      (node) => node.type === MessageActions,
    );
    if (!isValidElement<ComponentProps<typeof MessageActions>>(actions))
      throw new Error("Expected message actions");
    return createElement(
      "div",
      null,
      usage,
      createElement(MessageInfo, required(actions.props.info)),
    );
  });
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const tab = (key: string) =>
    required(
      document.querySelector<HTMLButtonElement>(
        `[role="tab"][data-account-key="${key}"]`,
      ),
    );
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(
            ServerAreasContextProvider,
            { value: areas },
            createElement(Caller),
          ),
        ),
      ),
    );
    await act(async () => required(native?.openLimits)());
    expect(usageProps?.opened).toBe(true);
    const firstRequest = required(usageProps).openRequest;
    await act(async () => tab("account-B").click());
    expect(tab("account-B").dataset.selected).toBe("true");
    expect(required(native).planType).toBe("plus");
    expect(required(native).recovery?.title).toBe("Recovery A");
    await act(async () =>
      required(
        host.querySelector<HTMLButtonElement>(
          '[aria-label="Message information"]',
        ),
      ).click(),
    );
    const info = required(
      document.querySelector(
        '[role="dialog"][aria-label="Message information"]',
      ),
    );
    expect(info.textContent).toContain("Account A");
    expect(info.textContent).not.toContain("Account B");
    reads.mockClear();
    await act(async () => required(native?.refreshLimits)());
    expect(reads).toHaveBeenCalledWith(
      "/api/limits",
      expect.objectContaining({ query: { account_key: "account-A" } }),
    );
    await act(async () => required(native?.openLimits)());
    expect(usageProps?.openRequest).toBe((firstRequest ?? 0) + 1);
    expect(usageProps?.opened).toBe(true);
    expect(tab("account-A").dataset.selected).toBe("true");
    expect(tab("account-B").dataset.selected).toBeUndefined();
  } finally {
    await act(async () => root.unmount());
    host.remove();
    models.store.dispose();
    areas.scope.services.close();
  }
});

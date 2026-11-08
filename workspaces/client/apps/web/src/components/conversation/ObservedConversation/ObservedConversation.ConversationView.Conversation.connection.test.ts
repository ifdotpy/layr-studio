import { AgentSource, AgentStatus } from "../../../generated/api.ts";
// @vitest-environment happy-dom
import { TranscriptHistory } from "../../../conversation/historySession.ts";
import { selectMessageConnection } from "../../../hooks/messageConnection.ts";
import type { Agent } from "../../../types.ts";
import { useConversationActions } from "../support/navigation.ts";
import { conversationActionsFixture } from "../../shell/integration-support/observableFixture.tsx";
vi.mock("../support/navigation.ts", async () => {
  const { createArea } = await import("../../../app/area.ts");
  const [TranscriptProvider, useTranscriptActions] =
    createArea<
      import("../support/navigation.ts").ConversationTranscriptActions
    >("Transcript");
  return {
    useConversationActions: vi.fn(),
    TranscriptProvider,
    useTranscriptActions,
  };
});
vi.mock("../support/requestView.ts", () => ({
  createRequestsView: () => ({ requests: [] }),
}));
vi.mock("../../agents/support/state.ts", async (original) => ({
  ...(await original<typeof import("../../agents/support/state.ts")>()),
  useAgentCatalog: () => ({ agents: [] }),
}));
import type {
  ConnectionActivity,
  ConnectionHealth,
} from "../../../sync/connectionState.ts";
import { required } from "../ComposerAttachments/componentTestFixtures.ts";
import { afterEach, expect, it, vi } from "vitest";
import type { ComponentProps, ReactElement } from "react";
import type { ResourceConnectionDetail } from "../../../sync/resourceConnection.ts";

const h = vi.hoisted(() => ({
  health: "live" as ConnectionHealth,
  activity: "open" as ConnectionActivity,
  detail: {} as ResourceConnectionDetail,
  online: false,
  feedback: false,
  store: undefined as
    | ReturnType<typeof import("../../../store/server.ts").createServerStore>
    | undefined,
  cursor: 0,
  state: [] as unknown[],
  effects: [] as Array<() => unknown>,
  dirty: false,
}));
vi.mock("../../../usePendingCommands.ts", () => ({
  usePendingCommands: () => [],
}));
vi.mock("react", async (original) => ({
  ...(await original<typeof import("react")>()),
  useState: (initial: unknown) => {
    const initialValue = () =>
      typeof initial === "function" ? initial() : initial;
    if (!h.feedback) return [initialValue(), vi.fn()];
    const index = h.cursor++;
    if (!(index in h.state)) h.state[index] = initialValue();
    return [
      h.state[index],
      (next: unknown) => {
        const value = typeof next === "function" ? next(h.state[index]) : next;
        if (!Object.is(h.state[index], value)) h.dirty = true;
        h.state[index] = value;
      },
    ];
  },
  useRef: (current: unknown) => ({ current }),
  useMemo: (compute: () => unknown) => compute(),
  useCallback: (callback: unknown) => callback,
  useLayoutEffect: () => {},
  useEffect: (effect: () => unknown) => {
    if (h.feedback) h.effects.push(effect);
  },
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerStore: () => h.store!,
    useServerApi: () => ({ get: vi.fn(), post: vi.fn() }),
    useServerOutbox: () => ({}),
    useServerUploads: () => ({}),
  }),
);
vi.mock("../../../conversation/liveText.ts", async (original) => ({
  ...(await original<typeof import("../../../conversation/liveText.ts")>()),
  useLiveText: () => new Map(),
}));
vi.mock("../../../servers/activity.ts", async (original) => ({
  ...(await original<typeof import("../../../servers/activity.ts")>()),
  useServerActivity: () => {},
}));
vi.mock("@mantine/hooks", async (original) => ({
  ...(await original<typeof import("@mantine/hooks")>()),
  useMediaQuery: () => false,
  useNetwork: () => ({ online: h.online }),
}));
vi.mock("../support/useQuestionCards.ts", () => ({
  createQuestionCards: () => () => undefined,
}));
vi.mock("../support/usePromptRecall.ts", () => ({
  usePromptRecall: () => ({}),
}));
vi.mock("../support/useConversationScroll.ts", () => ({
  useConversationScroll: () => ({
    scroll: { current: null },
    content: { current: null },
  }),
}));
vi.mock("../support/useAttachmentDrafts.ts", () => ({
  useAttachmentDrafts: () => [{}, vi.fn()],
}));
vi.mock("../support/useUploadRecovery.ts", () => ({
  useUploadRecovery: () => ({ pending: [] }),
}));
vi.mock("../support/useChatReadState.ts", () => ({
  useVisibleChatResult: () => {},
}));
vi.mock("../support/useDisplayPhase.ts", () => ({
  useDisplayPhase: () => "thinking",
}));
vi.mock("../../agents/support/WorkerModelPicker.tsx", () => ({
  useWorkerModels: () => ({ models: [] }),
}));
vi.mock("../support/useMessageQueue.ts", () => ({
  useMessageQueue: () => ({ items: [] }),
}));
vi.mock("../support/useSendingMessageRemoval.ts", () => ({
  useSendingMessageRemoval: () => ({}),
}));
vi.mock("../../../wasm/client.ts", () => ({}));
vi.mock("../../../hooks/conversationData", () => ({
  useChatWait: () => undefined,
}));
import { createElement, isValidElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ConversationView as Conversation } from "../ConversationView/ConversationView.tsx";
import { LiveAgentPhase } from "../../agents/AgentPhase/AgentPhase.tsx";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import { localTime } from "../../../local-time.ts";
import { DeliveryOwner } from "../../../app/conversation/DeliveryOwner.ts";
import type { OutgoingMessage } from "../../../sync/send.ts";

const agent = {
  id: "lead",
  source: AgentSource.managed,
  status: AgentStatus.running,
  activity: { phase: "thinking" },
} satisfies Agent;
const saved: OutgoingMessage = {
  id: "message-1",
  body: { id: "message-1", room: "lead", text: "Hello" },
  status: {
    source: "device",
    state: { kind: "transport_failed", error: "Failed to fetch" },
  },
  created: 1,
};

let feedbackFixture: ReturnType<typeof conversationActionsFixture> | undefined;
let feedbackOwner: DeliveryOwner | undefined;
let feedbackEntries: OutgoingMessage[] = [],
  feedbackErrors: string[] = [];
function feedback(entries: OutgoingMessage[], errors: string[] = []) {
  feedbackEntries = entries;
  feedbackErrors = errors;
  feedbackFixture ??= conversationActionsFixture();
  feedbackFixture.navigation.selectedId = "lead";
  feedbackOwner ??= feedbackFixture.delivery;
  feedbackFixture.startup.startupError = feedbackErrors[0] || "";
  feedbackOwner.entries = feedbackEntries;
  feedbackOwner.online = h.online;
  feedbackOwner.retainDiagnostics();
  return feedbackOwner;
}
function phaseElements(
  node: ReactNode,
): ReactElement<ComponentProps<typeof LiveAgentPhase>>[] {
  if (Array.isArray(node)) return node.flatMap(phaseElements);
  if (!isValidElement<{ children?: ReactNode }>(node)) return [];
  if (
    isValidElement<ComponentProps<typeof LiveAgentPhase>>(node) &&
    node.type === LiveAgentPhase
  )
    return [node];
  return phaseElements(node.props.children);
}
function renderActivity(
  pending: boolean,
  currentAgent: Agent | undefined = agent,
) {
  const fixture = observableFixture(currentAgent ? [currentAgent] : []);
  h.store = fixture.store;
  const actions = conversationActionsFixture(() => {}, fixture.store);
  actions.navigation.selectedId = "lead";
  if (!(actions.selected.history instanceof TranscriptHistory))
    throw new Error("Expected transcript history");
  actions.selected.history.connection = selectMessageConnection(
    h.health,
    h.activity,
    h.detail,
  );
  actions.delivery.entries = pending ? [saved] : [];
  actions.delivery.online = !pending;
  actions.delivery.retainDiagnostics();
  actions.delivery.consumed = ["Failed to fetch"];
  vi.mocked(useConversationActions).mockReturnValue(actions);
  const tree = Conversation();
  const phases = phaseElements(tree);
  expect(phases).toHaveLength(1);
  expect(required(phases[0]).props.savedMessagePending).toBe(pending);
  expect(required(phases[0]).props.connectionDetails).toBe("Failed to fetch");
  return renderToStaticMarkup(
    createElement(
      fixture.Provider,
      {},
      createElement(LiveAgentPhase, required(phases[0]).props),
    ),
  );
}
afterEach(() => {
  feedbackFixture?.scope.services.close();
  feedbackFixture = undefined;
  feedbackOwner = undefined;
  h.health = "live";
  h.activity = "open";
  h.detail = {};
  h.online = false;
  h.feedback = false;
  h.state = [];
  h.effects = [];
  vi.unstubAllGlobals();
});

it("renders a saved message with neutral text and icon through the conversation while transport is live", () => {
  expect(feedback([saved]).offlinePending).toBe(true);
  h.online = true;
  const retained = feedback([saved]);
  expect(retained.offlinePending).toBe(true);
  const markup = renderActivity(retained.offlinePending);
  expect(markup).toContain("Your message is saved and waiting for delivery.");
  expect(markup).toContain("lucide-clock");
  expect(markup).not.toContain("Connection lost");
  expect(markup).not.toContain("lucide-wifi-off");
  expect(saved.status).toEqual({
    source: "device",
    state: { kind: "transport_failed", error: "Failed to fetch" },
  });
});
it("keeps actual retry activity beside the saved-message notice", () => {
  h.health = "degraded";
  h.activity = "connecting";
  h.detail = { failure: "stream-error" };
  const markup = renderActivity(
    feedback([saved], ["Failed to fetch"]).offlinePending,
  );
  expect(markup).toContain(
    "Connection failed. Reconnecting… Your message is saved and waiting for delivery.",
  );
  expect(markup).toContain("lucide-loader-circle");
  expect(markup).toContain("Failed to fetch");
});
it("retains the reconnecting notice without a saved message", () => {
  h.health = "degraded";
  h.activity = "connecting";
  h.detail = { failure: "stream-error" };
  const markup = renderActivity(false);
  expect(markup).toContain("Reconnecting…");
  expect(markup).toContain("lucide-loader-circle");
  expect(markup).not.toContain("<details>");
});
it.each([
  ["preparing", "Preparing connection…", "lucide-clock"],
  ["waiting-owner", "Waiting for the shared connection…", "lucide-clock"],
  ["retry-scheduled", "Reason unknown. Retry scheduled.", "lucide-clock"],
  ["connecting", "Reason unknown. Reconnecting…", "lucide-loader-circle"],
  ["open", "Reason unknown. Waiting for server updates…", "lucide-clock"],
  ["offline", "No network.", "lucide-wifi-off"],
  ["stopped", "Reason unknown. Connection interrupted.", "lucide-wifi-off"],
] as const)(
  "renders %s activity through the Conversation caller",
  (activity, copy, icon) => {
    h.health = "degraded";
    h.activity = activity;
    const markup = renderActivity(false);
    expect(markup).toContain(copy);
    expect(markup).toContain(icon);
    if (activity !== "connecting") expect(markup).not.toContain("Reconnecting");
    expect(markup).not.toContain("<details>");
    expect(markup).toContain('role="status"');
  },
);
it("shows no network even while the last validated server health is live", () => {
  h.health = "live";
  h.activity = "offline";
  const markup = renderActivity(false);
  expect(markup).toContain("No network.");
  expect(markup).not.toContain("Reconnecting");
  expect(markup).toContain("lucide-wifi-off");
});
it.each([
  ["stream-error", "Connection failed."],
  ["heartbeat-timeout", "Server updates timed out."],
  ["schema-handshake-timeout", "Server compatibility check timed out."],
  ["subscription-timeout", "Server updates did not start in time."],
  ["subscription-rejected", "Could not confirm live updates."],
  ["invalid-frame", "Received invalid server updates."],
  ["local-state-error", "Could not prepare or apply local sync data."],
  ["unknown", "Reason unknown."],
] as const)(
  "renders the known %s cause and scheduled retry time",
  (failure, cause) => {
    h.health = "degraded";
    h.activity = "retry-scheduled";
    h.detail = { failure, retryAt: 1_792_965_000_000 };
    const markup = renderActivity(false);
    expect(markup).toContain(cause);
    expect(markup).toContain(
      `Retry scheduled for ${localTime(new Date(h.detail.retryAt!), { hour: "2-digit", minute: "2-digit", second: "2-digit" })}.`,
    );
    expect(markup).not.toContain("Reconnecting");
    expect(markup).not.toContain("<details>");
  },
);
it.each([true, false])(
  "keeps intentional paused transport neutral (saved=%s)",
  (pending) => {
    h.health = "paused";
    h.activity = "paused";
    const markup = renderActivity(pending);
    expect(markup).not.toContain("Connection lost");
    expect(markup).not.toContain("lucide-wifi-off");
    expect(markup).toContain(
      pending ? "Your message is saved and waiting for delivery." : "Thinking",
    );
  },
);
it("retains normal compact activity when there is no delivery notice", () => {
  const markup = renderActivity(false);
  expect(markup).toContain("Thinking");
  expect(markup).toContain("lucide-brain");
  expect(markup).not.toContain("Your message is saved");
});
it("retains terminal transcript errors after delivery instead of duplicating the phase banner", () => {
  const failed: Agent = {
    ...agent,
    status: AgentStatus.failed,
    error: "Provider failed",
  };
  expect(renderActivity(false, failed)).toBe("");
});

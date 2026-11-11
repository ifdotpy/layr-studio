// @vitest-environment happy-dom
import { TranscriptHistory } from "../../../conversation/historySession.ts";
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
import { afterEach, expect, it, vi } from "vitest";
import { isValidElement, type ReactElement, type ReactNode } from "react";

const h = vi.hoisted(() => ({
  effects: [] as Array<() => unknown>,
  refs: [] as Array<{ current: unknown }>,
  refIndex: 0,
  showLatest: vi.fn(async () => {}),
  root: {
    scrollTop: 0,
    scrollHeight: 1000,
    clientHeight: 100,
    dispatchEvent: (event: Event) => {
      if (event.type === "studio-history-page-anchor")
        (event as CustomEvent<{ id: string | null }>).detail.id = "visible";
      return true;
    },
    querySelectorAll: () => ({ length: 0, item: () => null }),
  },
  older: vi.fn(
    async (capture?: () => { id: string | null; endId: string | null }) => {
      capture?.();
    },
  ),
  newer: vi.fn(
    async (capture?: () => { id: string | null; endId: string | null }) => {
      capture?.();
    },
  ),
  anchor: vi.fn(),
  consume: vi.fn(),
  follow: vi.fn(),
}));
vi.mock("../support/useConversationScroll.ts", async (original) => ({
  ...(await original<typeof import("../support/useConversationScroll.ts")>()),
  createConversationScroll: () => ({
    scroll: { current: h.root },
    content: { current: null },
    getFollow: () => true,
    subscribeFollow: () => () => {},
    setFollow: h.follow,
    onScroll: () => {},
    hasRecentScrollInput: () => true,
    consumeHistoryScrollInput: h.consume,
    remember: () => {},
    revealRequest: () => {},
    getAnchorId: () => null,
  }),
}));
vi.mock("react", async (original) => ({
  ...(await original<typeof import("react")>()),
  useState: (initial: unknown) => [
    typeof initial === "function" ? initial() : initial,
    vi.fn(),
  ],
  useRef: (current: unknown) => {
    const index = h.refIndex++;
    return (h.refs[index] ||= { current });
  },
  useMemo: (compute: () => unknown) => compute(),
  useCallback: (callback: unknown) => callback,
  useLayoutEffect: () => {},
  useEffect: (effect: () => unknown) => {
    h.effects.push(effect);
  },
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerStore: () => ({
      state: () => ({
        current: {
          status: "live",
          data: {
            agents: new Map(),
            workspaces: new Map([["workspace", { stateDir: "fixture" }]]),
          },
        },
      }),
    }),
    useServerApi: () => ({ get: vi.fn(), post: vi.fn() }),
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
vi.mock("../support/usePromptRecall.ts", () => ({
  usePromptRecall: () => ({}),
}));
vi.mock("../support/useChatReadState.ts", () => ({
  useVisibleChatResult: () => {},
}));
vi.mock("../../agents/support/WorkerModelPicker.tsx", () => ({
  useWorkerModels: () => ({ models: [] }),
}));
vi.mock("../../../wasm/client.ts", () => ({}));
import { renderConversationViewFixture as Conversation } from "../ConversationView/ConversationView.fixture.tsx";

type Props = { id: string };
const propsFixture: Props = {
  id: "lead",
};
function elements(node: ReactNode): ReactElement<{
  children?: ReactNode;
  id?: string;
  onScroll?: () => void;
}>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (
    !isValidElement<{
      children?: ReactNode;
      id?: string;
      onScroll?: () => void;
    }>(node)
  )
    return [];
  return [node, ...elements(node.props.children)];
}
let selectedAreas: ReturnType<typeof conversationActionsFixture> | undefined;
function renderConversation(props: Props) {
  h.refIndex = 0;
  vi.spyOn(TranscriptHistory.prototype, "older").mockImplementation(h.older);
  vi.spyOn(TranscriptHistory.prototype, "newer").mockImplementation(h.newer);
  vi.spyOn(TranscriptHistory.prototype, "setPageAnchor").mockImplementation(
    h.anchor,
  );
  vi.spyOn(TranscriptHistory.prototype, "showLatest").mockImplementation(
    h.showLatest,
  );
  const actions = (selectedAreas ??= conversationActionsFixture());
  actions.navigation.selectedId = props.id;
  Object.assign(actions.selected.history, {
    before: "older",
    after: "newer",
  });
  vi.mocked(useConversationActions).mockReturnValue(actions);
  return Conversation();
}
afterEach(() => {
  vi.restoreAllMocks();
  vi.clearAllMocks();
  h.consume.mockReset();
  h.root.clientHeight = 100;
  h.root.scrollHeight = 1000;
  h.effects = [];
  h.refs = [];
  h.refIndex = 0;
  selectedAreas = undefined;
});
it.each([
  [50, "older"],
  [850, "newer"],
] as const)(
  "loads %s near the edge once per input and preserves the visible anchor",
  async (top, direction) => {
    h.root.scrollTop = top;
    h.consume.mockReturnValueOnce(true).mockReturnValue(false);
    const tree = renderConversation(propsFixture);
    const nodes = elements(tree);
    expect(
      nodes.some(
        (node) =>
          node.props.id === "earlier-messages" ||
          node.props.id === "newer-messages",
      ),
    ).toBe(false);
    const scroll = nodes.find((node) => node.props.id === "messages")!;
    (
      scroll.props as typeof scroll.props & {
        onWheel: (event: { deltaY: number }) => void;
      }
    ).onWheel({ deltaY: direction === "older" ? -30 : 30 });
    await new Promise((resolve) => setTimeout(resolve, 0));
    scroll.props.onScroll!();
    expect(h[direction]).toHaveBeenCalledOnce();
    expect(h.anchor).toHaveBeenCalledWith("visible");
    expect(h.follow).toHaveBeenCalledWith(false);
  },
);

it("uses outward input at an unmoving bottom edge and chooses newer in overlapping zones", () => {
  h.root.scrollHeight = 1000;
  h.root.clientHeight = 600;
  h.root.scrollTop = 400;
  h.consume.mockReturnValue(true);
  const tree = renderConversation(propsFixture);
  const scroll = elements(tree).find((node) => node.props.id === "messages")!;
  const props = scroll.props as typeof scroll.props & {
    onWheel: (event: { deltaY: number }) => void;
  };
  props.onWheel({ deltaY: 30 });
  expect(h.newer).toHaveBeenCalledOnce();
  expect(h.older).not.toHaveBeenCalled();
});

it.each(["touch", "keyboard"] as const)(
  "loads older from a clamped edge on %s input",
  (input) => {
    h.root.scrollTop = 0;
    h.consume.mockReturnValue(true);
    const tree = renderConversation(propsFixture);
    const scrollProps = elements(tree).find(
      (node) => node.props.id === "messages",
    )!.props;
    const props = scrollProps as typeof scrollProps & {
      onTouchStart: (event: { touches: { clientY: number }[] }) => void;
      onTouchMove: (event: { touches: { clientY: number }[] }) => void;
      onKeyDown: (event: {
        target: { closest: () => null };
        key: string;
        shiftKey: boolean;
      }) => void;
    };
    if (input === "touch") {
      props.onTouchStart({ touches: [{ clientY: 10 }] });
      props.onTouchMove({ touches: [{ clientY: 30 }] });
    } else
      props.onKeyDown({
        target: { closest: () => null },
        key: "ArrowUp",
        shiftKey: false,
      });
    expect(h.older).toHaveBeenCalledOnce();
  },
);

it("does not block another chat while the previous page is pending", async () => {
  const pending = Promise.withResolvers<void>();
  h.older.mockImplementationOnce(() => pending.promise);
  h.consume.mockReturnValue(true);
  h.root.scrollTop = 0;
  const first = elements(renderConversation(propsFixture)).find(
    (node) => node.props.id === "messages",
  )!;
  (
    first.props as typeof first.props & {
      onWheel: (event: { deltaY: number }) => void;
    }
  ).onWheel({ deltaY: -1 });
  const second = elements(
    renderConversation({ ...propsFixture, id: "other" }),
  ).find((node) => node.props.id === "messages")!;
  (
    second.props as typeof second.props & {
      onWheel: (event: { deltaY: number }) => void;
    }
  ).onWheel({ deltaY: -1 });
  expect(h.older).toHaveBeenCalledTimes(2);
  pending.resolve();
});

it("does not follow after a latest request completes for a different chat", async () => {
  const pending = Promise.withResolvers<void>();
  h.showLatest.mockImplementationOnce(() => pending.promise);
  const first = elements(renderConversation(propsFixture)).find((node) => {
    const type: unknown = node.type;
    return (
      typeof type === "object" &&
      type !== null &&
      "type" in type &&
      typeof type.type === "function" &&
      type.type.name === "FollowLatest"
    );
  });
  if (
    !first ||
    !("onClick" in first.props) ||
    typeof first.props.onClick !== "function"
  )
    throw new Error("Expected the observed FollowLatest action");
  first.props.onClick();
  renderConversation({ ...propsFixture, id: "other" });
  pending.resolve();
  await pending.promise;
  await Promise.resolve();
  expect(h.follow).not.toHaveBeenCalledWith(true);
});

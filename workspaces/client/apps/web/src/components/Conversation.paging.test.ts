import { afterEach, expect, it, vi } from "vitest";
import {
  isValidElement,
  type ComponentProps,
  type ReactElement,
  type ReactNode,
} from "react";

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
vi.mock("../usePendingCommands", () => ({ usePendingCommands: () => [] }));
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
vi.mock("../servers/ServerServicesContext", async (original) => ({
  ...(await original<typeof import("../servers/ServerServicesContext")>()),
  useServerApi: () => ({ get: vi.fn(), post: vi.fn() }),
  useServerOutbox: () => ({}),
  useServerUploads: () => ({}),
}));
vi.mock("../conversation/liveText", async (original) => ({
  ...(await original<typeof import("../conversation/liveText")>()),
  useLiveText: () => new Map(),
}));
vi.mock("../servers/activity", async (original) => ({
  ...(await original<typeof import("../servers/activity")>()),
  useServerActivity: () => {},
}));
vi.mock("@mantine/hooks", async (original) => ({
  ...(await original<typeof import("@mantine/hooks")>()),
  useMediaQuery: () => false,
}));
vi.mock("../hooks", () => ({
  useMessages: () => ({
    items: [],
    loaded: true,
    before: "older",
    after: "newer",
    older: h.older,
    newer: h.newer,
    setPageAnchor: h.anchor,
    showLatest: h.showLatest,
  }),
}));
vi.mock("./useMessageReceipts", () => ({
  useMessageReceipts: () => new Map(),
}));
vi.mock("./questions/useQuestionCards", () => ({
  useQuestionCards: () => () => undefined,
}));
vi.mock("./removedMessages", () => ({
  useRemovedMessages: () => ({ restored: [], hidden: () => false }),
}));
vi.mock("./usePromptRecall", () => ({ usePromptRecall: () => ({}) }));
vi.mock("./useConversationScroll", () => ({
  useConversationScroll: () => ({
    scroll: { current: h.root },
    content: { current: null },
    onScroll: vi.fn(),
    setFollow: h.follow,
    getAnchorId: () => "visible",
    consumeHistoryScrollInput: h.consume,
  }),
}));
vi.mock("./useAttachmentDrafts", () => ({
  useAttachmentDrafts: () => [{}, vi.fn()],
}));
vi.mock("./useUploadRecovery", () => ({
  useUploadRecovery: () => ({ pending: [] }),
}));
vi.mock("./useChatReadState", () => ({ useVisibleChatResult: () => {} }));
vi.mock("./useDisplayPhase", () => ({ useDisplayPhase: () => "error" }));
vi.mock("./agents/WorkerModelPicker", () => ({
  useWorkerModels: () => ({ models: [] }),
}));
vi.mock("./useMessageQueue", () => ({
  useMessageQueue: () => ({ items: [] }),
}));
vi.mock("./useSendingMessageRemoval", () => ({
  useSendingMessageRemoval: () => ({}),
}));
vi.mock("../wasm/client", () => ({}));
import Conversation from "./Conversation";

type Props = ComponentProps<typeof Conversation>;
const propsFixture: Partial<Props> = {
  id: "lead",
  data: {
    stateDir: "fixture",
    threads: [],
    runtime: { requests: [], agents: [], projects: [], nativeNotices: [] },
  },
  getDraft: () => "",
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
function renderConversation(props: Props) {
  h.refIndex = 0;
  return Conversation(props);
}
afterEach(() => {
  vi.clearAllMocks();
  h.consume.mockReset();
  h.root.clientHeight = 100;
  h.root.scrollHeight = 1000;
  h.effects = [];
  h.refs = [];
  h.refIndex = 0;
});
it.each([
  [50, "older"],
  [850, "newer"],
] as const)(
  "loads %s near the edge once per input and preserves the visible anchor",
  async (top, direction) => {
    h.root.scrollTop = top;
    h.consume.mockReturnValueOnce(true).mockReturnValue(false);
    const tree = renderConversation(propsFixture as Props);
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
  const tree = renderConversation(propsFixture as Props);
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
    const tree = renderConversation(propsFixture as Props);
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
  const first = elements(renderConversation(propsFixture as Props)).find(
    (node) => node.props.id === "messages",
  )!;
  (
    first.props as typeof first.props & {
      onWheel: (event: { deltaY: number }) => void;
    }
  ).onWheel({ deltaY: -1 });
  const second = elements(
    renderConversation({ ...propsFixture, id: "other" } as Props),
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
  const first = elements(renderConversation(propsFixture as Props)).find(
    (node) =>
      typeof node.type === "function" && node.type.name === "FollowLatest",
  )!;
  (first.props as typeof first.props & { onClick: () => void }).onClick();
  renderConversation({ ...propsFixture, id: "other" } as Props);
  pending.resolve();
  await pending.promise;
  await Promise.resolve();
  expect(h.follow).not.toHaveBeenCalledWith(true);
});

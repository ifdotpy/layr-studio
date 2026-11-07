import { resourceData } from "../store/runtime.ts";
import { useServerStore } from "../servers/ServerServicesContext.tsx";
import { agentId as resourceAgentId } from "../generated/observableResources.ts";
import {
  ConversationProvider,
  useConversationActions,
} from "./conversation/navigation.ts";
import { useAgentCatalog } from "./agents/state.ts";
import { observer } from "mobx-react-lite";
import { serverStatus } from "../messageSendStatus";
import { MessageSendStatus } from "../generated/api.ts";
import {
  AgentSource as GeneratedApiAgentSource,
  AgentStatus as GeneratedApiAgentStatus,
  RoomKind as GeneratedApiRoomKind,
} from "../generated/api.ts";
import { randomUUID } from "../uuid";
import { codePointsExceed, sendsTextFile } from "../commands";
import { ProviderKind, providerSupports } from "../generated/providerKind.ts";
import type { ReactNode } from "react";
import { IdleCompactionNotice } from "./IdleCompactionNotice";
import {
  isCompactTools,
  resolveLayoutLength,
} from "../layout/deriveWorkspaceLayout";
import { layoutQueries } from "../layout/layout-contract.generated";
import { usePrimaryTouch } from "../hooks/touchCapability";
import {
  MAX_TURN_ATTACHMENTS,
  MAX_DRAFT_TEXT_CODEPOINTS,
} from "../generated/sharedValues";
import { DeliveryMoment } from "../generated/deliveryMoment";
import { useSendingMessageRemoval } from "./useSendingMessageRemoval";
import {
  useServerApi,
  useServerOutbox,
} from "../servers/ServerServicesContext";
import { useServerActivity } from "../servers/activity";
import { computed, compareShallow } from "mobx";
import { useConversationHistory } from "../conversation/historySession";
import AgentAvatar from "./agents/AgentAvatar";
import MessageQueue, {
  type LocalQueueItem,
  type QueueItem,
} from "./MessageQueue";
import { useMessageQueue } from "./useMessageQueue";
import { useMessageReceipts } from "./useMessageReceipts";
import { receiptTranscript } from "../sync/messageReceipts";
import { useVisibleChatResult } from "./useChatReadState";
import {
  NativeError,
  NativeNotice,
  isNonBlockingWarning,
} from "./NativeNotice";
import { ActionIcon, Button, Loader, Modal, Textarea } from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { usePendingCommands } from "../usePendingCommands";
import { createPortal } from "react-dom";
import {
  ArrowDown,
  ArrowUp,
  ListTree,
  Trash2,
  Square,
  ListEnd,
  Ellipsis,
} from "lucide-react";
import {
  useCallback,
  useLayoutEffect,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { errorText } from "../api";
import FullOutput from "./conversation/transcript/FullOutput";
import SafetyBuffering from "./conversation/transcript/SafetyBuffering";
import { currentCapacityRetry } from "../capacityRetry";
import { isCyberPolicyRefusal, nativeThreadError } from "../nativeErrors";
import {
  transcriptMessages,
  useRetainedArray,
  useCommittedCallback,
} from "../hooks";
import DraftVersions from "./DraftVersions";
import type { DraftVersion } from "../sync/drafts";
import { type OutgoingMessage } from "../sync/send";
import { useDisplayPhase } from "./useDisplayPhase";
import {
  outgoingTranscript,
  deliveryLabel,
  messageRenderKey,
  explicitQueue,
  dispatchedMessage,
  mergeQueueOrder,
} from "./message-delivery/messageDelivery";
import { useRemovedMessages } from "./removedMessages";
import { isSendingMessage } from "./removeSendingMessages";

import { useConversationScroll } from "./useConversationScroll";
import { useAttachmentDrafts } from "./useAttachmentDrafts";
import { useUploadRecovery } from "./useUploadRecovery";
import { useServerUploads } from "../servers/ServerServicesContext";
import {
  type Agent,
  type Json,
  type KnownApiFields,
  type Message,
  type Room,
} from "../types";
import Usage from "./Usage";
import type { UsageAccount } from "./Usage";
import type { AccountLimitsData } from "../usage/accountUsage";
import PromptNavigator from "./PromptNavigator";
import { usePromptRecall } from "./usePromptRecall";
import PromptComposer from "./prompt-composer/PromptComposer";
import PromptInput from "./prompt-composer/PromptInput";
import { reportPromptComposerRender } from "./prompt-composer/renderProbe";
import Requests, { RequestArea, RequestCard } from "./questions/Requests";
import { createRequestsView } from "./questions/requestView";
import { createQuestionCards } from "./questions/useQuestionCards";
import MessageDate from "./conversation/transcript/MessageDate";
import ClaudeRepairNotice from "./ClaudeRepairNotice";
import TurnHistory from "./conversation/transcript/TurnHistory";
import {
  resetHistoryWindowAnchor,
  revealHistoryMessage,
} from "./conversation/transcript/historyWindowModel";
import { isEmptyAssistantMessage } from "./turnHistoryModel";
import { Dictation } from "./Dictation";
import RealtimeVoice from "./RealtimeVoice";
import OutboxControls from "./OutboxControls";
import SettledDelivery from "./message-delivery/SettledDelivery";
import StreamingText from "./conversation/transcript/StreamingText";
import SelectionQuote, {
  selectedExcerpt,
} from "./conversation/transcript/SelectionQuote";
import MessageActions from "./conversation/transcript/MessageActions";
import { LiveAgentPhase } from "./agents/AgentPhase";
import ConversationSettings from "./prompt-composer/ConversationSettings";
import { AccountTransferChip } from "./Accounts";
import ComposerAttachments, {
  MessageAttachments,
  type Attachment,
} from "./ComposerAttachments";
import { LiveConversationStart } from "./conversation/ConversationStart";

import "./chat-controls.css";
import "./conversation/empty-chat-settings.css";
import { copyText } from "../clipboard/clipboard";
import { transcriptRequests } from "./questions/questionCardModel";

const modelCommandPattern = /^\/model(?:\s|$)/i;
const exactModelCommandPattern = /^\/model$/i;

function isJsonObject(value: unknown): value is Json {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

const MessageRow = observer(function MessageRow({
  render,
  item,
}: {
  render: (item: Message) => ReactNode;
  item: Message;
}) {
  return render(item);
});

type PageStatus = {
  id: string | null | undefined;
  direction: "older" | "newer";
  failed: boolean;
} | null;
type PageStatusStore = { value: PageStatus; listeners: Set<() => void> };
function PageIndicator({
  store,
  id,
  direction,
  retry,
}: {
  store: PageStatusStore;
  id: string | null | undefined;
  direction: "older" | "newer";
  retry: () => void;
}) {
  const status = useSyncExternalStore(
    (listener) => {
      store.listeners.add(listener);
      return () => store.listeners.delete(listener);
    },
    () => store.value,
    () => store.value,
  );
  if (!status || status.id !== id || status.direction !== direction)
    return null;
  return status.failed ? (
    <Button size="compact-sm" onClick={retry}>
      Retry loading {direction === "older" ? "earlier" : "later"} messages
    </Button>
  ) : (
    <p className="transcript-page-link" role="status">
      Loading messages…
    </p>
  );
}

function FollowLatest(p: {
  subscribe: (listener: () => void) => () => void;
  getSnapshot: () => boolean;
  onClick: () => void;
}) {
  const following = useSyncExternalStore(
    p.subscribe,
    p.getSnapshot,
    p.getSnapshot,
  );
  if (following) return null;
  return (
    <div className="jump-slot">
      <Button
        id="jump-latest"
        className="jump"
        variant="default"
        radius="xl"
        leftSection={<ArrowDown size={14} />}
        onClick={p.onClick}
      >
        Latest
      </Button>
    </div>
  );
}

// The root style also carries values that change every frame, such as the
// sidebar width while it is dragged. Only the typography inputs change the main
// font size, and the inline style reads them without a style recalculation, so
// the computed size is read again only when they change.
let mainFontKey: string | undefined;
let mainFontPx = Number.NaN;
function mainFontSize() {
  const root = document.documentElement;
  const key = `${root.dataset.studioTypography}|${root.style.getPropertyValue("--studio-main-font-size")}`;
  if (key !== mainFontKey) {
    mainFontKey = key;
    mainFontPx = Number.parseFloat(
      getComputedStyle(root).getPropertyValue("--studio-main-font-size"),
    );
  }
  return mainFontPx;
}

function useCompactHeaderTools() {
  const isCompact = (workspaceWidth?: number) =>
    window.matchMedia(layoutQueries.navigationCompact).matches ||
    isCompactTools(
      workspaceWidth ??
        document
          .querySelector<HTMLElement>(".workspace")
          ?.getBoundingClientRect().width ??
        window.innerWidth,
      mainFontSize(),
    );
  const [compact, setCompact] = useState(() =>
    typeof window === "undefined" ? false : isCompact(),
  );
  useEffect(() => {
    const viewport = window.matchMedia(layoutQueries.navigationCompact);
    const workspace = document.querySelector<HTMLElement>(".workspace");
    let width: number | undefined;
    const update = () => setCompact(isCompact(width));
    // A root style change matters only when it changes the typography inputs.
    const preferences = new MutationObserver(() => {
      if (mainFontKey === undefined) return update();
      const before = mainFontKey;
      mainFontSize();
      if (mainFontKey !== before) update();
    });
    // The observer reports the new width; reading it again would force layout.
    const workspaceObserver = workspace
      ? new ResizeObserver((entries) => {
          width = entries.at(-1)?.borderBoxSize[0]?.inlineSize ?? width;
          update();
        })
      : undefined;
    if (workspace) workspaceObserver?.observe(workspace);
    viewport.addEventListener("change", update);
    preferences.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["style", "data-studio-typography"],
    });
    update();
    return () => {
      viewport.removeEventListener("change", update);
      workspaceObserver?.disconnect();
      preferences.disconnect();
    };
  }, []);
  return compact;
}

const emptyAttachments: Attachment[] = [];

export function ConversationView(p: {
  id: string | null;
  visible?: boolean;
  headerToolsInMenu?: boolean | undefined;
  onRailToolsChange?: ((tools: ReactNode) => void) | undefined;
  transcriptHeading?: ReactNode;
  syncWorkspaceId?: string | undefined;
  agent?: Agent | undefined;
  detail?:
    | import("../generated/observableResources.ts").AgentDetailEntityDto
    | undefined;
  lead?: Agent | undefined;
  room?: Room | undefined;
  legacy?: Json | undefined;
  draftConflicts?: DraftVersion[];
  outgoing?: OutgoingMessage[];
  offlineSendPending?: boolean;
  offlineSendDetails?: string;
  sending: boolean;
  schemaMismatch?: boolean;
  limits: AccountLimitsData | null;
  limitsAccounts?: UsageAccount[];
  limitsLoading?: boolean;
  jumpTarget?: { messageId: string; requestId: string } | undefined;
  limitsAccountLabel?: string | undefined;
}) {
  const actions = useConversationActions();
  const store = useServerStore();
  const stateDir =
    resourceData(store.state().current)?.workspaces.values().next().value
      ?.stateDir || "";
  const agents = useAgentCatalog().agents;
  const { removeSendingMessage } = useSendingMessageRemoval();
  const { queueUploads } = useServerUploads();
  const { get, post, command } = useServerApi();
  const { changeOutbox } = useServerOutbox();

  reportPromptComposerRender("conversation");
  const limitsData = isJsonObject(p.limits?.data) ? p.limits.data : undefined;
  const rateLimits = isJsonObject(limitsData?.rateLimits)
    ? limitsData.rateLimits
    : undefined;
  const planType =
    typeof rateLimits?.planType === "string" ? rateLimits.planType : undefined;
  const mobileClient = useMediaQuery(layoutQueries.navigationCompact);
  // A touch keyboard's Enter adds a new line at every width; the button sends.
  const touchInput = usePrimaryTouch();
  const kind = p.room ? "room" : p.legacy ? "legacy" : "agent";
  const compactHeaderTools = useCompactHeaderTools();
  const [headerTools, setHeaderTools] = useState<HTMLElement | null>(null);
  const [toolsExpanded, setToolsExpanded] = useState(false);
  const [toolsMenuPosition, setToolsMenuPosition] = useState<{
    left: number;
    top: number;
    width: number;
  } | null>(null);
  useLayoutEffect(() => {
    setHeaderTools(
      p.headerToolsInMenu
        ? null
        : document.getElementById("conversation-header-tools"),
    );
  }, [p.headerToolsInMenu]);
  useEffect(() => {
    if (!compactHeaderTools) setToolsExpanded(false);
  }, [compactHeaderTools]);
  useEffect(() => {
    if (!compactHeaderTools || !toolsExpanded || !headerTools) return;
    const menu = headerTools.querySelector(".conversation-header-tools-menu");
    if (!menu) return;
    const dismissOutside = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      if (menu.contains(target) || target.closest(".mantine-Popover-dropdown"))
        return;
      setToolsExpanded(false);
    };
    const dismissEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setToolsExpanded(false);
    };
    document.addEventListener("pointerdown", dismissOutside, true);
    document.addEventListener("keydown", dismissEscape, true);
    return () => {
      document.removeEventListener("pointerdown", dismissOutside, true);
      document.removeEventListener("keydown", dismissEscape, true);
    };
  }, [compactHeaderTools, headerTools, toolsExpanded]);
  useLayoutEffect(() => {
    if (!compactHeaderTools || !toolsExpanded || !headerTools) {
      setToolsMenuPosition(null);
      return;
    }
    const summary = headerTools.querySelector(
      ".conversation-header-tools-summary",
    );
    if (!summary) return;
    const updatePosition = () => {
      const anchor = summary.getBoundingClientRect();
      const rootFontPx = Number.parseFloat(
        getComputedStyle(document.documentElement).fontSize,
      );
      const compactToolsMenuMaxWidthPx = resolveLayoutLength(
        "tools.menuMax",
        rootFontPx,
      );
      const compactToolsMenuViewportGutterPx = resolveLayoutLength(
        "tools.menuGutter",
        rootFontPx,
      );
      const compactToolsMenuGapPx = resolveLayoutLength(
        "tools.menuGap",
        rootFontPx,
      );
      const viewportWidth =
        document.documentElement.clientWidth || window.innerWidth;
      const menuWidth = Math.min(
        compactToolsMenuMaxWidthPx,
        Math.max(1, viewportWidth - compactToolsMenuViewportGutterPx * 2),
      );
      const minLeft = compactToolsMenuViewportGutterPx;
      const maxLeft = Math.max(minLeft, viewportWidth - menuWidth - minLeft);
      setToolsMenuPosition({
        left: Math.max(minLeft, Math.min(anchor.left, maxLeft)),
        top: anchor.bottom + compactToolsMenuGapPx,
        width: menuWidth,
      });
    };
    updatePosition();
    const observer = new ResizeObserver(updatePosition);
    observer.observe(summary);
    const workspace = headerTools.closest<HTMLElement>(".workspace");
    if (workspace) observer.observe(workspace);
    window.addEventListener("resize", updatePosition);
    window.visualViewport?.addEventListener("resize", updatePosition);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", updatePosition);
      window.visualViewport?.removeEventListener("resize", updatePosition);
    };
  }, [compactHeaderTools, headerTools, toolsExpanded]);
  const historySession = useConversationHistory(
    p.id,
    kind,
    p.agent?.source === GeneratedApiAgentSource.managed,
    stateDir,
    p.syncWorkspaceId,
    p.visible ?? true,
  );
  const {
    items: history,
    historyVersion,
    notice,
    before,
    older,
    newer,
    setPageAnchor,
    after,
    historical,
    pageLoading,
    showLatest,
    ensureMessage,
    connection,
    loaded,
  } = historySession;
  const outgoing = (p.outgoing || []).filter(
    (entry) => entry.body.room === p.id,
  );
  const removed = useRemovedMessages(stateDir, kind, p.id);
  const receiptIds = [
    ...new Set([
      ...outgoing
        .filter(
          (entry) =>
            (entry.status.source === "device" ||
              serverStatus(entry.status) === MessageSendStatus.queued ||
              serverStatus(entry.status) === MessageSendStatus.sending ||
              serverStatus(entry.status) === MessageSendStatus.uncertain) &&
            (entry.receipt?.status !== MessageSendStatus.delivered ||
              entry.receipt.materialized !== true),
        )
        .map((entry) => entry.id),
      ...[...history, ...removed.restored]
        .filter(
          (item) =>
            item.role === "user" &&
            (item.deliveryStatus?.source === "device" ||
              serverStatus(item.deliveryStatus) === MessageSendStatus.queued ||
              serverStatus(item.deliveryStatus) === MessageSendStatus.sending ||
              serverStatus(item.deliveryStatus) ===
                MessageSendStatus.uncertain),
        )
        .map(
          (item) =>
            item.clientMessageId ||
            (item.id.startsWith(p.id + ":")
              ? item.id.slice(p.id!.length + 1)
              : item.id),
        ),
    ]),
  ].sort();
  const receipts = useMessageReceipts(
    p.agent?.source === GeneratedApiAgentSource.managed ? p.id : null,
    `${stateDir}:${p.syncWorkspaceId}:${p.id}`,
    p.syncWorkspaceId,
    receiptIds,
  );
  const queueScope = `${stateDir}:${p.syncWorkspaceId || ""}:${p.id}`;
  const messageQueue = useMessageQueue({
    id: p.id,
    enabled: p.agent?.source === GeneratedApiAgentSource.managed,
    scope: queueScope,
    workspaceId: p.syncWorkspaceId,
    observed: actions.observeSends,
    edited: actions.editSend,
    refresh: actions.refresh,
  });
  const questionAgentId =
    p.agent?.source === GeneratedApiAgentSource.managed && !p.room
      ? p.agent.id
      : undefined;
  const requestArea = useMemo(() => {
    const requests = createRequestsView(store, {
      agentId: p.id,
      familyId: p.room?.rootId,
      includeTeam: !mobileClient,
    });
    return {
      requests,
      cards: createQuestionCards(store, requests, questionAgentId),
    };
  }, [store, p.id, p.room?.rootId, mobileClient, questionAgentId]);
  const deliveryTranscript = useMemo(
    () =>
      computed(() =>
        outgoingTranscript(
          receiptTranscript(history, p.id || "", receipts),
          (p.outgoing || []).filter((entry) => entry.body.room === p.id),
        ),
      ),
    [history, p.outgoing, p.id, receipts],
  ).get();
  const visibleMessages = useMemo(
    () =>
      computed(() =>
        receiptTranscript(
          [
            ...deliveryTranscript.items.map((message) => {
              if (
                !isSendingMessage(message) ||
                message.deliveryStatus?.source === "server"
              )
                return message;
              // Keep the device removal fact until the server supplies a status.
              return (
                removed.restored.find(
                  (copy) =>
                    message.id === copy.id ||
                    (copy.clientMessageId &&
                      copy.clientMessageId === message.clientMessageId),
                ) || message
              );
            }),
            ...removed.restored.filter(
              (copy) =>
                !deliveryTranscript.items.some(
                  (message) =>
                    message.id === copy.id ||
                    (copy.clientMessageId &&
                      copy.clientMessageId === message.clientMessageId),
                ),
            ),
          ].filter((message) => !removed.hidden(message)),
          p.id || "",
          receipts,
        ),
      ),
    [
      deliveryTranscript.items,
      removed.hidden,
      removed.restored,
      p.id,
      receipts,
    ],
  ).get();
  const historyAgentId = p.id;
  const promptRecall = usePromptRecall(
    `${stateDir}:${kind}:${p.id}:${historyVersion}`,
    p.room ? [] : visibleMessages,
    () => actions.getDraft(p.id || "new"),
    actions.setDraft,
    p.agent?.source === GeneratedApiAgentSource.managed &&
      kind === "agent" &&
      historyAgentId
      ? {
          before: before ? String(before) : null,
          load: async (cursor) => {
            const page = await get("/api/transcript/page", {
              query: { id: historyAgentId, before: cursor, limit: 500 },
            });
            if (page.unavailable) throw new Error(page.unavailable);
            if (
              historyVersion &&
              page.historyVersion &&
              historyVersion !== page.historyVersion
            )
              throw new Error(
                "The conversation changed. Open its message history again.",
              );
            return {
              messages: transcriptMessages(
                page.items || [],
                historyAgentId,
              ).filter((item) => !removed.hidden(item)),
              before: page.nextCursor || null,
            };
          },
          onError: (error) => actions.notify(errorText(error)),
        }
      : undefined,
  );
  const observed = deliveryTranscript.observed.join(",");
  useEffect(() => {
    if (observed) actions.observeSends?.(observed.split(","));
  }, [observed, actions.observeSends]);
  const {
    scroll,
    content,
    getFollow,
    subscribeFollow,
    setFollow,
    onScroll,
    hasRecentScrollInput,
    consumeHistoryScrollInput,
    remember,
    revealRequest,
    getAnchorId,
  } = useConversationScroll(
    `${stateDir}:${kind}:${p.id}`,
    loaded,
    !after,
    `studio-turns:${stateDir}:${p.id}`,
  );
  const pageStatus = useRef<PageStatusStore>({
    value: null,
    listeners: new Set(),
  });
  const setPageStatus = (value: PageStatus) => {
    pageStatus.current.value = value;
    pageStatus.current.listeners.forEach((listener) => listener());
  };
  const paging = useRef<{ id: string | null | undefined } | null>(null);
  const loadPage = (direction: "older" | "newer") => {
    if (paging.current?.id === p.id) return;
    const request = { id: p.id };
    paging.current = request;
    setPageStatus({ id: p.id, direction, failed: false });
    setFollow(false);
    const captureAnchor = () => {
      setFollow(false);
      const detail: { id: string | null; endId: string | null } = {
        id: null,
        endId: null,
      };
      scroll.current?.dispatchEvent(
        new CustomEvent("studio-history-page-anchor", { detail }),
      );
      const id = detail.id;
      setPageAnchor(id);
      return detail;
    };
    void (direction === "older" ? older(captureAnchor) : newer(captureAnchor))
      .then(() => {
        if (paging.current === request) setPageStatus(null);
      })
      .catch((error) => {
        if (paging.current === request && activeId.current === request.id) {
          setPageStatus({ id: p.id, direction, failed: true });
          actions.notify(errorText(error));
        }
      })
      .finally(() => {
        if (paging.current === request) paging.current = null;
      });
  };
  const lastHistoryTop = useRef(0);
  const touchHistoryY = useRef<number | null>(null);
  const pageAtEdge = (direction: "older" | "newer") => {
    const root = scroll.current;
    if (
      !root ||
      paging.current?.id === p.id ||
      (pageStatus.current.value?.id === p.id && pageStatus.current.value.failed)
    )
      return;
    const near =
      direction === "older"
        ? before && root.scrollTop <= root.clientHeight
        : after &&
          root.scrollHeight - root.scrollTop - root.clientHeight <=
            root.clientHeight;
    if (near && consumeHistoryScrollInput()) loadPage(direction);
  };
  const onHistoryScroll = () => {
    const top = scroll.current?.scrollTop ?? 0;
    const direction = top < lastHistoryTop.current ? "older" : "newer";
    lastHistoryTop.current = top;
    onScroll();
    pageAtEdge(direction);
  };
  const input = useRef<HTMLTextAreaElement>(null);
  const navigationAttempt = useRef(0);
  const handledJump = useRef("");
  const [highlighted, setHighlighted] = useState<string | null>(null);
  const navigateToMessage = async (id: string) => {
    const root = scroll.current;
    if (!root) return;
    resetHistoryWindowAnchor(root);
    const chat = p.id;
    const attempt = ++navigationAttempt.current;
    setFollow(false);
    setPageAnchor(getAnchorId());
    const available = await ensureMessage(id);
    if (activeId.current !== chat || navigationAttempt.current !== attempt)
      return;
    if (!available)
      throw new Error("This message is unavailable in this conversation.");
    for (let frame = 0; frame < 20; frame++) {
      if (
        scroll.current !== root ||
        activeId.current !== chat ||
        navigationAttempt.current !== attempt
      )
        return;
      revealHistoryMessage(root, id);
      const target: HTMLElement | undefined = Array.from(
        root.querySelectorAll<HTMLElement>("[data-message]"),
      ).find(
        (element) =>
          element.dataset.message === id ||
          element.dataset.sourceMessage === id,
      );
      if (target) {
        let container: HTMLElement | null = target.parentElement;
        while (container && container !== root) {
          if (container instanceof HTMLDetailsElement) container.open = true;
          container = container.parentElement;
        }
        if (!target.hasAttribute("data-lazy-message")) {
          revealRequest(target, 16);
          setHighlighted(target.dataset.message || null);
          return;
        }
      }
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => resolve()),
      );
    }
    throw new Error("The message could not be displayed. Search again.");
  };
  const jumpToPrompt = useCommittedCallback((id: string) => {
    void navigateToMessage(id).catch((error) =>
      actions.notify(errorText(error)),
    );
  });
  const returnToLatest = () => {
    if (scroll.current) resetHistoryWindowAnchor(scroll.current);
    const attempt = ++navigationAttempt.current;
    const chat = p.id;
    setHighlighted(null);
    void showLatest()
      .then(() => {
        if (activeId.current === chat && navigationAttempt.current === attempt)
          setFollow(true);
      })
      .catch((error) => actions.notify(errorText(error)));
  };
  useEffect(() => {
    if (!p.jumpTarget || handledJump.current === p.jumpTarget.requestId) return;
    const target = p.jumpTarget;
    handledJump.current = target.requestId;
    void navigateToMessage(target.messageId).catch((error) =>
      actions.notify(errorText(error)),
    );
  }, [p.jumpTarget?.requestId, p.id]);
  const attachmentKey = `codex-agent-attachments:${stateDir}`;
  const [attachments, setAttachments] = useAttachmentDrafts(
    attachmentKey,
    actions.notify,
  );
  const [addingFiles, setUploading] = useState(false);
  const uploadRecovery = useUploadRecovery(
    stateDir,
    p.syncWorkspaceId || "",
    setAttachments,
  );
  const pendingFiles = useRetainedArray(
    uploadRecovery.pending.filter((row) => row.agent === p.id),
  );
  const uploading = addingFiles || pendingFiles.length > 0;
  useServerActivity(addingFiles || uploadRecovery.pending.length > 0);
  const [limitsOpen, setLimitsOpen] = useState(false);
  const sendLock = useRef<symbol | null>(null);
  const latestSend = useRef<Record<string, symbol>>({});
  const branchLock = useRef(false);
  const branchRequests = useRef<Record<string, string>>({});
  const [branchDraft, setBranchDraft] = useState<{
    message: Message;
    before: boolean;
    text: string;
  } | null>(null);
  const [branching, setBranching] = useState(false);
  const [editLoading, setEditLoading] = useState<string | null>(null);
  const editAttempt = useRef(0);
  const [dragging, setDragging] = useState(false);
  const [stopping, setStopping] = useState(false);
  const stopAttempt = useRef(0);
  // A stop stays shown until the stream holds the row that the command changed.
  const pendingCommands = usePendingCommands();
  const stopShown =
    stopping ||
    pendingCommands.some(
      (entry) => entry.kind === "stop" && entry.target === p.id,
    );
  useEffect(() => {
    stopAttempt.current += 1;
    setStopping(false);
  }, [p.id]);
  const uploadLock = useRef(false);
  const activeId = useRef(p.id);
  activeId.current = p.id;
  const assets = attachments[p.id || ""] || emptyAttachments;
  const agent = p.agent;
  const detail = p.detail;
  const controls = {
    steer: providerSupports(agent?.provider, "steer"),
    fork: providerSupports(agent?.provider, "fork"),
  };
  useVisibleChatResult(
    scroll,
    p.agent,
    visibleMessages,
    loaded,
    actions.observeRead,
    p.syncWorkspaceId,
    !historical && !after,
  );
  const threadBlock = nativeThreadError(agent, detail);
  const capacityRetry = agent ? currentCapacityRetry(agent) : null;
  const displayPhase = useDisplayPhase(
    p.id,
    detail?.activity?.phase || "thinking",
  );
  const detailedActivity =
    !!threadBlock ||
    capacityRetry?.status === "scheduled" ||
    !!connection?.message ||
    !!p.offlineSendPending ||
    !!detail?.error ||
    !!detail?.startAttempt?.prepareError ||
    !!detail?.startAttempt?.responseError ||
    ["retrying", "auth", "safety", "error"].includes(
      detail?.activity?.phase || "",
    ) ||
    !["starting", "running", "queued", "idle", "completed"].includes(
      agent?.status || "idle",
    );
  const reasoningVisible =
    detail?.activity?.phase === "thinking" &&
    visibleMessages.at(-1)?.turnId === agent?.turnId &&
    visibleMessages.at(-1)?.role === "reasoning" &&
    typeof visibleMessages.at(-1)?.reasoningSince === "number";
  useEffect(() => {
    setBranchDraft(null);
    editAttempt.current++;
    setEditLoading(null);
    setHighlighted(null);
    setLimitsOpen(false);
  }, [p.id]);
  const managed = agent?.source === GeneratedApiAgentSource.managed;
  const [modelCommandRequest, setModelCommandRequest] = useState({
    id: p.id,
    serial: 0,
  });
  useEffect(() => {
    setModelCommandRequest({ id: p.id, serial: 0 });
  }, [p.id]);

  // The queue holds only next-turn messages (Tab). Other input is
  // delivered at once and stays in the chat while it is sent.
  const pendingQueueEntries = useMemo(
    () =>
      computed<QueueItem[]>(() =>
        [
          ...messageQueue.items,
          ...visibleMessages
            .filter(
              (item) =>
                item.role === "user" &&
                explicitQueue(item) &&
                !dispatchedMessage(item) &&
                !messageQueue.items.some(
                  (row) =>
                    row.id === item.clientMessageId ||
                    row.id === item.id ||
                    `${p.id}:${row.id}` === item.id,
                ),
            )
            .map((item): LocalQueueItem => ({
              id: item.clientMessageId || item.id,
              text: item.text,
              ...(item.assets ? { assets: item.assets } : {}),
              requestedDelivery: item.requestedDelivery,
              localDelivery: true,
              deliveryStatus: item.deliveryStatus,
            })),
        ].filter(
          (entry) =>
            (("localDelivery" in entry && entry.localDelivery === true) ||
              ("status" in entry &&
                entry.status === MessageSendStatus.queued)) &&
            !visibleMessages.some(
              (item) =>
                item.role === "user" &&
                dispatchedMessage(item) &&
                (item.clientMessageId === entry.id ||
                  item.id === entry.id ||
                  item.id === `${p.id}:${entry.id}`),
            ),
        ),
      ),
    [messageQueue.items, visibleMessages, p.id],
  ).get();
  const { nextTurnQueue, currentTurnQueue } = useMemo(
    () =>
      computed(() => {
        const isNextTurn = (entry: QueueItem) =>
          (("delivery" in entry ? entry.delivery : undefined) ||
            entry.requestedDelivery) === DeliveryMoment.NextTurn;
        return {
          nextTurnQueue: pendingQueueEntries.filter(isNextTurn),
          currentTurnQueue: pendingQueueEntries.filter(
            (entry) => !isNextTurn(entry),
          ),
        };
      }),
    [pendingQueueEntries],
  ).get();
  const addFiles = useCommittedCallback(async (files: globalThis.File[]) => {
    if (!p.id || !managed)
      throw new Error("Select a managed chat before attaching files.");
    if (uploadLock.current)
      throw new Error("Wait for the selected files to be saved, then retry.");
    if (!files.length) throw new Error("Select at least one file.");
    if (
      files.length + assets.length + pendingFiles.length >
      MAX_TURN_ATTACHMENTS
    )
      throw new Error(`Attach at most ${MAX_TURN_ATTACHMENTS} files.`);
    const id = p.id;
    uploadLock.current = true;
    setUploading(true);
    try {
      uploadRecovery.include(
        await queueUploads(stateDir, id, files, p.syncWorkspaceId || ""),
      );
      uploadRecovery.retry();
    } finally {
      uploadLock.current = false;
      setUploading(false);
    }
  });
  const submit = useCommittedCallback(
    async (
      delivery: DeliveryMoment = DeliveryMoment.CurrentTurn,
      asTextFile: boolean = false,
    ) => {
      const draft = actions.getDraft(p.id || "new");
      const draftTooLong = codePointsExceed(draft, MAX_DRAFT_TEXT_CODEPOINTS);
      const modelCommand = modelCommandPattern.test(draft.trim());
      const exactModelCommand = exactModelCommandPattern.test(draft.trim());
      if (!sendsTextFile(draft, asTextFile) && modelCommand) {
        if (!exactModelCommand) {
          actions.notify("Use /model without arguments to choose a model.");
        } else if (!managed || !agent) {
          actions.notify("Select a managed chat to choose a model.");
        } else {
          promptRecall.reset();
          input.current?.focus({ preventScroll: true });
          actions.setDraft("");
          setModelCommandRequest((value) => ({
            id: p.id,
            serial: value.serial + 1,
          }));
        }
        return;
      }
      if (
        sendLock.current ||
        p.sending ||
        threadBlock ||
        uploading ||
        draftTooLong ||
        (!draft.trim() && !assets.length)
      )
        return;
      const attempt = Symbol();
      sendLock.current = attempt;
      const id = p.id || "";
      latestSend.current[id] = attempt;
      const release = () => {
        if (sendLock.current === attempt) sendLock.current = null;
      };
      // Sending expresses a new scroll intent. Streaming still respects a later
      // manual scroll away from the bottom.
      returnToLatest();
      input.current?.focus({ preventScroll: true });
      const sent = new Set(assets.map((asset) => asset.id));
      try {
        await actions.send({
          ...(asTextFile ? { asTextFile: true } : {}),
          delivery,
          assets: assets.map((asset) => asset.id),
          attachments: assets.map(({ preview: _preview, ...asset }) => asset),
          onPersist: async () => {
            await setAttachments((current) => ({
              ...current,
              [id]: (current[id] || []).filter((asset) => !sent.has(asset.id)),
            }));
            release();
          },
        });
        if (p.id && managed)
          void messageQueue
            .reload()
            .catch((error) => actions.notify(errorText(error)));
      } catch {
        setAttachments((current) =>
          latestSend.current[id] !== attempt
            ? current
            : {
                ...current,
                [id]: Array.from(
                  new Map(
                    [...assets, ...(current[id] || [])].map((asset) => [
                      asset.id,
                      asset,
                    ]),
                  ).values(),
                ),
              },
        );
        // The send owner restores the draft and renders its delivery error.
      } finally {
        release();
      }
    },
  );
  const editMessage = useCommittedCallback(async (message: Message) => {
    const attempt = ++editAttempt.current;
    const chat = p.id;
    setEditLoading(message.id);
    try {
      let original = message;
      if (message.truncated) {
        const full = await get("/api/transcript/item", {
          query: { id: chat || "", message_id: message.id },
          timeoutMs: 15000,
        });
        if (full.truncated || typeof full.text !== "string")
          throw new Error(
            "The full original message is unavailable. Copy the visible text into a new message instead.",
          );
        const fullFields: KnownApiFields<typeof full> = full;
        original = {
          ...message,
          ...fullFields,
          deliveryStatus: fullFields.deliveryStatus
            ? { source: "server", status: fullFields.deliveryStatus }
            : message.deliveryStatus,
          threadId: fullFields.threadId ?? message.threadId,
          role: fullFields.role || message.role,
          text:
            typeof fullFields.text === "string"
              ? fullFields.text
              : message.text,
        };
      }
      if (activeId.current === chat && attempt === editAttempt.current)
        setBranchDraft({
          message: original,
          before: true,
          text: original.text,
        });
    } catch (error) {
      if (activeId.current === chat && attempt === editAttempt.current)
        actions.notify(errorText(error));
    } finally {
      if (attempt === editAttempt.current) setEditLoading(null);
    }
  });
  const branch = useCommittedCallback(
    async (message: Message, draft?: { before: boolean; text: string }) => {
      if (
        visibleMessages.some(
          (item) =>
            item.turnId === message.turnId && item.turnStatus === "uncertain",
        )
      ) {
        actions.notify(
          "The turn outcome is unconfirmed. Review the saved evidence before you continue.",
        );
        return;
      }
      if (branchLock.current) return;
      const agentId = p.id;
      if (!agentId) return;
      branchLock.current = true;
      const sourceId = draft?.before
        ? message.id
        : message.sourceId || message.id;
      const key = `${agentId}:${sourceId}:${draft?.before ? "before" : "after"}`;
      branchRequests.current[key] ||= randomUUID();
      setBranching(true);
      try {
        const response = await post("/api/branch", {
          agent: agentId,
          message_id: sourceId,
          id: branchRequests.current[key],
          ...(draft?.before ? { before: true } : {}),
        });
        const branchId = response.id;
        if (!branchId)
          throw new Error("The server did not return the new chat identity.");
        if (draft) {
          const reviewedText =
            draft.before &&
            draft.text === message.text &&
            typeof response.draft?.text === "string"
              ? response.draft.text
              : draft.text;
          const prefix =
            draft.before && typeof response.draft?.prefixText === "string"
              ? response.draft.prefixText
              : "";
          actions.setDraft(
            prefix ? `${prefix}\n\n${reviewedText}` : reviewedText,
            branchId,
          );
          const branchAssets = response.draft?.assets;
          if (draft.before && branchAssets?.length)
            setAttachments((current) => ({
              ...current,
              [branchId]: branchAssets,
            }));
        }
        await actions.refresh();
        (actions.branchCreated || actions.open)?.(branchId);
        setBranchDraft(null);
        delete branchRequests.current[key];
      } catch (error) {
        actions.notify(errorText(error));
      } finally {
        branchLock.current = false;
        setBranching(false);
      }
    },
  );
  const copy = useCommittedCallback(async (text: string) => {
    try {
      await copyText(text);
      actions.notify("Copied.");
    } catch {
      actions.notify("Clipboard access failed.");
    }
  });
  const quote = useCommittedCallback((text: string) => {
    const quoted = text
      .split(/\r?\n/)
      .map((line) => `> ${line}`)
      .join("\n");
    const draft = actions.getDraft(p.id || "new");
    const separator =
      !draft || draft.endsWith("\n\n")
        ? ""
        : draft.endsWith("\n")
          ? "\n"
          : "\n\n";
    actions.setDraft(`${draft}${separator}${quoted}\n\n`);
    input.current?.focus();
  });
  const first =
    p.room?.members?.[0] || visibleMessages.find((m) => m.sender)?.sender;
  const canSend =
    !p.schemaMismatch &&
    !threadBlock &&
    (!!detail?.canSend || !!p.legacy || !p.id);
  const lastAssistantByTurn = useMemo(
    () =>
      computed(() => {
        const uncertainTurns = new Set(
          visibleMessages
            .filter((message) => message.turnStatus === "uncertain")
            .map((message) => message.turnId),
        );
        return new Map(
          visibleMessages.flatMap((message) =>
            message.role === "assistant" &&
            message.turnId &&
            !uncertainTurns.has(message.turnId) &&
            !isEmptyAssistantMessage(message)
              ? [[message.turnId, message.id] as const]
              : [],
          ),
        );
      }),
    [visibleMessages],
  ).get();
  const queueEntry = (m: Message) =>
    m.role === "user"
      ? nextTurnQueue.find(
          (event) =>
            event.id === m.clientMessageId ||
            event.id === m.id ||
            `${p.id}:${event.id}` === m.id,
        )
      : undefined;
  const sendingEntry = (m: Message) =>
    m.role === "user"
      ? currentTurnQueue.find(
          (event) =>
            event.id === m.clientMessageId ||
            event.id === m.id ||
            `${p.id}:${event.id}` === m.id,
        )
      : undefined;
  const renderedMessages = useMemo(
    () =>
      computed(() =>
        [
          ...visibleMessages.filter((item) => !queueEntry(item)),
          ...currentTurnQueue
            .filter(
              (entry) =>
                !visibleMessages.some((item) => sendingEntry(item) === entry),
            )
            .map((entry): Message => ({
              id: entry.id,
              clientMessageId: entry.id,
              role: "user",
              text: entry.text,
              ...("assets" in entry ? { assets: entry.assets } : {}),
              localDelivery: true,
              deliveryStatus:
                "localDelivery" in entry && entry.localDelivery
                  ? entry.deliveryStatus
                  : { source: "server", status: entry.status },
            })),
        ].filter((message) => !removed.hidden(message)),
      ),
    [visibleMessages, nextTurnQueue, currentTurnQueue, p.id, removed.hidden],
  ).get();
  const questionCards = requestArea.cards;
  const inlineRequests = useMemo(
    () =>
      computed(
        () =>
          transcriptRequests(
            managed && !p.room ? renderedMessages : [],
            requestArea.requests.requests,
            questionCards,
          ),
        { equals: compareShallow },
      ),
    [managed, p.room, renderedMessages, requestArea, questionCards],
  ).get();
  const inlineRequestIds = new Set(
    [...inlineRequests.values()].flatMap((request) =>
      request ? [request.id] : [],
    ),
  );
  const [removingSending, setRemovingSending] = useState(false);
  const removeSending = useCommittedCallback(async (messages: Message[]) => {
    if (removingSending || !p.id) return;
    setRemovingSending(true);
    try {
      for (const message of messages)
        await removeSendingMessage(
          { stateDir: stateDir, workspaceId: p.syncWorkspaceId, kind },
          { chat: p.id, message },
        );
      void messageQueue.reload().catch(() => {});
      actions.notify(
        `Removed ${messages.length} messages. Work already sent continues.`,
      );
    } catch (error) {
      actions.notify(errorText(error));
    } finally {
      setRemovingSending(false);
    }
  });
  const restoreDraft = useCommittedCallback((m: Message) => {
    const draft = actions.getDraft(p.id || "new");
    const separator =
      draft.trim() && draft.trim() !== m.text.trim() ? "\n\n" : "";
    actions.setDraft(separator ? draft + separator + m.text : m.text);
    setAttachments((current) => ({
      ...current,
      [p.id || ""]: [
        ...new Map(
          [...(current[p.id || ""] || []), ...(m.assets || [])].map(
            (asset: Attachment) => [asset.id, asset],
          ),
        ).values(),
      ],
    }));
    input.current?.focus({ preventScroll: true });
  });
  const removeMessage = useCommittedCallback(async (m: Message) => {
    try {
      if (!m.clientMessageId) return;
      await changeOutbox(m.clientMessageId, "cancel");
      actions.observeSends?.([m.clientMessageId]);
      actions.notify(
        "Message removed from this device. Delivery was not cancelled.",
      );
    } catch (error) {
      actions.notify(`Could not remove this message: ${errorText(error)}`);
    }
  });
  const renderMessage = useCallback(
    (m: Message) => {
      reportPromptComposerRender("message", m.id);
      return renderMessageArticle(m);
    },
    [
      highlighted,
      p.room,
      p.id,
      first,
      managed,
      agent,
      controls.fork,
      removingSending,
      editLoading,
      branching,
      lastAssistantByTurn,
      p.outgoing,
      p.offlineSendPending,
      stateDir,
      p.limitsAccounts,
      agents,
      actions.notify,
      actions.observeSends,
    ],
  );
  const renderMessageArticle = (m: Message) => (
    <article
      key={messageRenderKey(m)}
      data-message={m.id}
      data-source-message={m.sourceId}
      data-found={highlighted === m.id || undefined}
      className={`message ${m.role === "user" ? "user" : "assistant"} ${p.room ? "bubble " + (m.sender !== first ? "outgoing" : "incoming") : ""}`}
    >
      {((m.deliveryStatus?.source === "device" &&
        (m.deliveryStatus.state.kind === "stored" ||
          m.deliveryStatus.state.kind === "in_flight")) ||
        serverStatus(m.deliveryStatus) === MessageSendStatus.sending) && (
        <SettledDelivery transient>
          <span
            className="message-delivery-status message-delivery-inline"
            role="status"
          >
            {deliveryLabel(m)}
            {m.role === "user" && (
              <ActionIcon
                size="sm"
                aria-label="Remove sending message"
                title="Remove from this device and stop retries. Work already sent continues."
                disabled={removingSending}
                onClick={() => void removeSending([m])}
              >
                <Trash2 size={14} />
              </ActionIcon>
            )}
          </span>
        </SettledDelivery>
      )}
      {/* The main chat has two speakers; only team rooms name each sender. */}
      {p.room && (
        <span className="chat-message-author">
          <AgentAvatar
            id={m.sender || p.agent?.id || p.id || "agent"}
            size={24}
          />
        </span>
      )}
      {deliveryLabel(m) &&
        !(
          m.deliveryStatus?.source === "device" &&
          (m.deliveryStatus.state.kind === "stored" ||
            m.deliveryStatus.state.kind === "in_flight")
        ) &&
        serverStatus(m.deliveryStatus) !== MessageSendStatus.sending && (
          <SettledDelivery
            transient={
              serverStatus(m.deliveryStatus) === MessageSendStatus.queued
            }
          >
            <div className="message-delivery-heading">
              {/* Delivery is the normal outcome: "Sent" stays for assistive
                  technology only, so history is not a column of "Sent". */}
              <span
                className={`message-label message-delivery-status${
                  serverStatus(m.deliveryStatus) === MessageSendStatus.delivered
                    ? " sr-only"
                    : ""
                }`}
                role="status"
              >
                {deliveryLabel(m)}
              </span>
              {m.role === "user" &&
                m.localDelivery &&
                (serverStatus(m.deliveryStatus) ===
                  MessageSendStatus.uncertain ||
                  serverStatus(m.deliveryStatus) === MessageSendStatus.failed ||
                  serverStatus(m.deliveryStatus) ===
                    MessageSendStatus.cancelled) && (
                  <ActionIcon
                    size="sm"
                    variant="subtle"
                    aria-label="Remove message"
                    title="Remove from this device. Delivery is not cancelled."
                    onClick={() => removeMessage(m)}
                  >
                    <Trash2 size={14} />
                  </ActionIcon>
                )}
            </div>
          </SettledDelivery>
        )}
      {m.role === "user" ? (
        <div className="prose plain">{m.text}</div>
      ) : (
        <StreamingText
          message={m}
          streaming={!!m.streaming}
          agentId={
            p.room
              ? agents.find((item) => item.id === m.sender)?.id
              : managed
                ? agent?.id
                : undefined
          }
        />
      )}
      {Array.isArray(m.assets) && (
        <MessageAttachments assets={m.assets} notify={actions.notify} />
      )}
      {m.localDelivery && (
        <OutboxControls
          entry={p.outgoing?.find((entry) => entry.id === m.clientMessageId)}
          onResolved={(id) => actions.observeSends?.([id])}
        />
      )}
      {m.deliveryError &&
        !(
          p.offlineSendPending &&
          m.localDelivery &&
          m.deliveryStatus?.source === "device" &&
          m.deliveryStatus.state.kind === "transport_failed"
        ) &&
        (serverStatus(m.deliveryStatus) === MessageSendStatus.failed ||
          serverStatus(m.deliveryStatus) === MessageSendStatus.uncertain ||
          (m.deliveryStatus?.source === "device" &&
            (m.deliveryStatus.state.kind === "transport_failed" ||
              m.deliveryStatus.state.kind === "receipt_expired"))) && (
          <div className="message-delivery-error">
            <p>{m.deliveryError}</p>
            {serverStatus(m.deliveryStatus) === MessageSendStatus.failed && (
              <Button
                size="compact-xs"
                variant="subtle"
                onClick={() => restoreDraft(m)}
              >
                Restore draft
              </Button>
            )}
          </div>
        )}
      {m.truncated && <p className="notice">This message is clipped.</p>}
      {m.fullTextBlob && <FullOutput blob={m.fullTextBlob} />}
      {(p.room || m.role === "user") && (
        <MessageDate at={m.at ?? m.created ?? m.timestamp} />
      )}
      <MessageActions
        hidden={!!m.streaming}
        info={
          m.role === "assistant"
            ? {
                message: m,
                agentId: !p.room && managed ? agent?.id : undefined,
                stateDir: stateDir,
                rootId:
                  !p.room && managed ? agent?.rootId || agent?.id : undefined,
                accounts: p.limitsAccounts,
                onCopy: (value) => void copy(value),
              }
            : undefined
        }
        onCopy={() => void copy(m.text)}
        onQuote={
          !p.room && !explicitQueue(m)
            ? () => {
                const excerpt = selectedExcerpt(scroll.current);
                quote(excerpt?.messageId === m.id ? excerpt.text : m.text);
                window.getSelection()?.removeAllRanges();
              }
            : undefined
        }
        onEdit={
          !p.room &&
          !explicitQueue(m) &&
          managed &&
          m.role === "user" &&
          m.turnId &&
          m.turnStatus !== "uncertain" &&
          (!m.deliveryStatus ||
            serverStatus(m.deliveryStatus) === MessageSendStatus.delivered)
            ? () => void editMessage(m)
            : undefined
        }
        editLoading={editLoading === m.id}
        editDisabled={branching || !!editLoading || m.turnId === agent?.turnId}
        onBranch={
          controls.fork &&
          !p.room &&
          !explicitQueue(m) &&
          managed &&
          m.role === "assistant" &&
          m.turnId &&
          m.turnId !== agent?.turnId &&
          m.turnStatus !== "uncertain" &&
          lastAssistantByTurn.get(m.turnId) === m.id
            ? () => void branch(m)
            : undefined
        }
        branchDisabled={m.turnId === agent?.turnId && !!agent?.inFlight}
        onAnotherAnswer={
          !p.room &&
          !explicitQueue(m) &&
          managed &&
          m.role === "assistant" &&
          m.turnId &&
          m.turnId !== agent?.turnId &&
          lastAssistantByTurn.get(m.turnId) === m.id
            ? () =>
                setBranchDraft({
                  message: m,
                  before: false,
                  text: "Give another answer to my previous request. Use the existing results. Do not run tools or commands unless I explicitly ask.",
                })
            : undefined
        }
        anotherAnswerDisabled={branching}
      />
    </article>
  );
  const renderTranscriptMessage = useCallback(
    (item: Message) =>
      item.idleCompaction ? (
        <IdleCompactionNotice key={item.id} text={item.text} />
      ) : item.nativeNotice ? (
        isNonBlockingWarning(item) ? null : (
          <NativeNotice key={item.id} item={item} planType={planType} />
        )
      ) : (
        <MessageRow
          key={messageRenderKey(item)}
          item={item}
          render={renderMessage}
        />
      ),
    [planType, renderMessage],
  );
  const requestCards = useMemo(
    () =>
      new Map(
        [...inlineRequests].map(([messageId, request]) => [
          messageId,
          request && (
            <RequestArea
              key={request.id}
              scope={stateDir}
              refresh={actions.refresh}
              notify={actions.notify}
              onAnswerOpen={() => setFollow(false)}
              onAnswerPosition={revealRequest}
            >
              <RequestCard
                request={request}
                messageId={messageId}
                showDates={!!p.room}
              />
            </RequestArea>
          ),
        ]),
      ),
    [
      inlineRequests,
      stateDir,
      actions.refresh,
      actions.notify,
      setFollow,
      revealRequest,
      p.room,
    ],
  );
  const transcriptActions = useMemo(
    () => ({
      ...actions,
      transcript: {
        renderMessage: renderTranscriptMessage,
        questionCards,
        requestCards,
        jumpToMessage: jumpToPrompt,
      },
    }),
    [
      actions.open,
      actions.nativeAction,
      actions.openActivity,
      renderTranscriptMessage,
      questionCards,
      requestCards,
      jumpToPrompt,
    ],
  );
  const transcript = (
    <ConversationProvider value={transcriptActions}>
      {managed && !p.room && agent?.provider === ProviderKind.Claude && (
        <ObservedClaudeRepairNotice agent={agent} detail={detail} />
      )}
      <TurnHistory
        key={stateDir + ":" + p.id}
        history={historySession}
        items={renderedMessages}
        scrollContainer={scroll}
        rememberScroll={remember}
        getFollow={getFollow}
        followTail={!after}
        hasRecentScrollInput={hasRecentScrollInput}
        agent={managed && !p.room ? agent : undefined}
        currentTurn={agent?.turnId || undefined}
        enabled={managed && !p.room}
        storageKey={"studio-turns:" + stateDir + ":" + p.id}
        agentId={managed ? agent?.id : undefined}
      />
    </ConversationProvider>
  );
  const promptNavigation = useMemo(
    () =>
      !p.room && p.id ? (
        <PromptNavigator
          compact
          inMenu={p.headerToolsInMenu}
          key={p.id}
          messages={visibleMessages}
          loadOlder={
            before
              ? () => {
                  setFollow(false);
                  setPageAnchor(getAnchorId());
                  void older().catch((e) => actions.notify(errorText(e)));
                }
              : undefined
          }
          loadingOlder={pageLoading}
          agentId={managed ? p.id || undefined : undefined}
          container={scroll}
          storageKey={`studio-prompt-bookmarks:${stateDir}:${p.id}`}
          jump={jumpToPrompt}
        />
      ) : null,
    [
      p.room,
      p.id,
      p.headerToolsInMenu,
      visibleMessages,
      before,
      pageLoading,
      managed,
      scroll,
      stateDir,
      jumpToPrompt,
      older,
      setFollow,
      getAnchorId,
      actions.notify,
    ],
  );
  useLayoutEffect(() => {
    p.onRailToolsChange?.(p.headerToolsInMenu ? promptNavigation : null);
  }, [p.headerToolsInMenu, p.onRailToolsChange, promptNavigation]);
  useLayoutEffect(
    () => () => p.onRailToolsChange?.(null),
    [p.onRailToolsChange],
  );
  // The composer renders again for every keystroke. Its parts that do not
  // read the draft are built here, with the chat, so React reuses them.

  const composer = useMemo(
    () =>
      computed(() => {
        const canStopAgent =
          !!agent &&
          (agent.status === GeneratedApiAgentStatus.running ||
            agent.status === GeneratedApiAgentStatus.starting ||
            agent.status === GeneratedApiAgentStatus.approval);
        const executionControls = managed && agent && (
          <ConversationSettings
            agent={agent}
            lead={p.lead}
            openRequest={
              modelCommandRequest.id === p.id ? modelCommandRequest.serial : 0
            }
          />
        );
        const attachTool = managed && (
          <ComposerAttachments
            notify={actions.notify}
            assets={assets}
            uploading={uploading}
            disabled={!canSend || p.sending}
            add={addFiles}
            remove={(id) => {
              const agentId = p.id || "";
              void uploadRecovery
                .remove(id, () => {
                  setAttachments((current) => ({
                    ...current,
                    [agentId]: (current[agentId] || []).filter(
                      (asset) => asset.id !== id,
                    ),
                  }));
                })
                .catch((error) => actions.notify(errorText(error)));
            }}
          />
        );
        const voiceTool = managed &&
          agent?.isLead &&
          agent?.provider === ProviderKind.Codex &&
          p.id &&
          !threadBlock && (
            <RealtimeVoice
              key={`voice:${p.id}`}
              agentId={p.id}
              notify={actions.notify}
            />
          );
        const transferChip = managed && p.agent?.isLead && (
          <ObservedAccountTransferChip
            transfer={p.agent.accountTransferProgress}
          />
        );
        const draftVersions = (
          <div className="composer-draft-control">
            <DraftVersions
              key={`drafts:${p.id || "new"}`}
              versions={p.draftConflicts || []}
              useVersion={(version, mode) =>
                actions.setDraft((current) =>
                  mode === "append" && current
                    ? `${current}\n\n${version.text}`
                    : version.text,
                )
              }
              dismiss={(version) => actions.dismissDraft?.(version)}
            />
          </div>
        );
        const pendingUploads = managed && (
          <>
            {!!pendingFiles.length && (
              <div className="attachment-list" role="status">
                {pendingFiles.map((file) => (
                  <div className="attachment-chip" key={file.id}>
                    <span>
                      {file.name}: saved on this device, waiting for upload
                    </span>
                    <Button
                      size="compact-xs"
                      onClick={() => {
                        void uploadRecovery
                          .remove(file.id, () => {
                            setAttachments((current) => ({
                              ...current,
                              [file.agent]: (current[file.agent] || []).filter(
                                (asset) => asset.id !== file.id,
                              ),
                            }));
                          })
                          .catch((error) => actions.notify(errorText(error)));
                      }}
                    >
                      Remove pending {file.name}
                    </Button>
                  </div>
                ))}
              </div>
            )}
            {uploadRecovery.error && (
              <p role="status">
                {uploadRecovery.error}{" "}
                <button type="button" onClick={uploadRecovery.retry}>
                  Retry file upload
                </button>
              </p>
            )}
          </>
        );
        const dictation = managed && p.id && (
          <Dictation
            key={`${stateDir}:${p.id}`}
            chatId={`${stateDir}:${p.id}`}
            disabled={!canSend || p.sending}
            onInsert={(text) =>
              actions.setDraft(
                (current) =>
                  current +
                  (current && !current.endsWith("\n") ? "\n" : "") +
                  text,
              )
            }
          />
        );
        const stopControl = (
          <>
            {!canStopAgent && (
              <span className="composer-stop-slot" aria-hidden="true" />
            )}
            {canStopAgent && (
              <ActionIcon
                type="button"
                id="stop"
                aria-label="Stop agent"
                title={canStopAgent ? "Stop agent" : "No active turn to stop"}
                disabled={stopShown || !canStopAgent}
                aria-busy={stopShown}
                onClick={() => {
                  const agentId = p.id;
                  if (stopShown || !agentId) return;
                  const attempt = ++stopAttempt.current;
                  setStopping(true);
                  void command
                    .stop(agentId, false)
                    .then(actions.refresh)
                    .catch((e) => actions.notify(errorText(e)))
                    .finally(() => {
                      if (stopAttempt.current === attempt) setStopping(false);
                    });
                }}
              >
                {stopShown ? (
                  <Loader size={14} color="currentColor" />
                ) : (
                  <Square size={14} fill="currentColor" />
                )}
              </ActionIcon>
            )}
          </>
        );

        return (
          <PromptComposer
            session={p.id || "new"}
            getDraft={actions.getDraft}
            subscribeDraft={actions.subscribeDraft}
            className={dragging ? "attachment-drop" : ""}
            onDragOver={(event) => {
              if (managed) {
                event.preventDefault();
                setDragging(true);
              }
            }}
            onDragLeave={(event) => {
              if (!event.currentTarget.contains(event.relatedTarget as Node))
                setDragging(false);
            }}
            onDrop={(event) => {
              event.preventDefault();
              setDragging(false);
              void addFiles(Array.from(event.dataTransfer.files)).catch(
                (error) => actions.notify(errorText(error)),
              );
            }}
            onSubmit={(e) => {
              e.preventDefault();
              void submit();
            }}
          >
            {(draft) => {
              const draftTooLong = codePointsExceed(
                draft,
                MAX_DRAFT_TEXT_CODEPOINTS,
              );
              const willAttachText = sendsTextFile(draft);
              const modelCommand =
                !willAttachText && modelCommandPattern.test(draft.trim());
              const exactModelCommand = exactModelCommandPattern.test(
                draft.trim(),
              );
              const queueControl = (className: string) =>
                managed ? (
                  <ActionIcon
                    className={className}
                    type="button"
                    variant="subtle"
                    aria-label="Queue for next turn"
                    title="Queue for the next turn (Tab)"
                    aria-keyshortcuts="Tab"
                    disabled={
                      !canSend ||
                      p.sending ||
                      uploading ||
                      draftTooLong ||
                      (!draft.trim() && !assets.length)
                    }
                    onClick={() => void submit(DeliveryMoment.NextTurn)}
                  >
                    <ListEnd size={18} />
                  </ActionIcon>
                ) : null;
              return (
                <>
                  {transferChip}
                  <PromptInput
                    value={draft}
                    session={p.id || "new"}
                    getDraft={actions.getDraft}
                    setDraft={actions.setDraft}
                    input={input}
                    mobile={mobileClient || touchInput}
                    managed={!!managed}
                    canSend={canSend}
                    blocked={!!threadBlock}
                    sending={p.sending}
                    uploading={uploading}
                    draftTooLong={draftTooLong}
                    modelCommand={modelCommand}
                    hasAttachments={!!assets.length}
                    onChange={(value) => {
                      promptRecall.reset();
                      actions.setDraft(value);
                    }}
                    onPasteFiles={(files) =>
                      void addFiles(files).catch((error) =>
                        actions.notify(errorText(error)),
                      )
                    }
                    onSend={() => void submit()}
                    onQueue={() => void submit(DeliveryMoment.NextTurn)}
                    onRecallKeyDown={promptRecall.onKeyDown}
                    skillCatalog={{
                      enabled:
                        !!managed && !p.room && !!p.id && p.id === agent?.id,
                      agentId: p.id || "",
                      workspace: p.syncWorkspaceId || stateDir,
                      account: p.agent?.accountKey || "default",
                      cwd: p.agent?.cwd || "",
                      provider: String(p.agent?.provider || ""),
                    }}
                  />
                  {exactModelCommand && (
                    <Button
                      type="button"
                      variant="subtle"
                      size="compact-sm"
                      aria-label="Choose model with /model"
                      onClick={() => void submit()}
                    >
                      /model · Choose model and reasoning
                    </Button>
                  )}
                  {willAttachText && (
                    <div
                      id="draft-length-error"
                      className="draft-length-error"
                      role="alert"
                    >
                      <span>
                        {draftTooLong
                          ? "Draft exceeds the size limit."
                          : "This text will be sent as a file."}
                      </span>
                      {managed && (
                        <Button
                          type="button"
                          size="compact-xs"
                          variant="subtle"
                          disabled={
                            !canSend ||
                            p.sending ||
                            uploading ||
                            draftTooLong ||
                            assets.length >= MAX_TURN_ATTACHMENTS
                          }
                          onClick={() =>
                            void submit(DeliveryMoment.CurrentTurn, true)
                          }
                        >
                          Attach text as a file
                        </Button>
                      )}
                    </div>
                  )}
                  <div className="composer-bar">
                    {draftVersions}
                    <div className="composer-tools">
                      {attachTool}
                      <div className="composer-tools-inline">{voiceTool}</div>
                      {managed && (
                        <details className="composer-tools-disclosure">
                          <summary
                            aria-label="Composer tools"
                            title="Voice conversation and queue"
                          >
                            <Ellipsis size={18} />
                          </summary>
                          <div className="composer-tools-popover">
                            {voiceTool}
                            {queueControl("composer-queue-disclosure")}
                          </div>
                        </details>
                      )}
                      {executionControls}

                      {pendingUploads}
                      {dictation}
                    </div>
                    <span id="send-state" role="status" aria-live="polite">
                      {p.sending ? "Sending…" : ""}
                    </span>
                    <div className="composer-submit-actions">
                      {managed && (
                        <span className="composer-timing">
                          <span>Send to current turn</span>
                          <span>Queue for next turn</span>
                        </span>
                      )}
                      {queueControl("composer-queue-primary")}
                      {/* The stop slot keeps its width while idle, so Stop
                          appearing during a turn moves no other control. */}
                      {stopControl}
                      <ActionIcon
                        type="submit"
                        variant="filled"
                        color="gray"
                        radius="xl"
                        id="send"
                        className="send"
                        disabled={
                          !canSend ||
                          p.sending ||
                          uploading ||
                          draftTooLong ||
                          (!draft.trim() && !assets.length)
                        }
                        aria-label="Send message"
                        title={
                          managed
                            ? "Send after tool calls (Enter)"
                            : "Send message"
                        }
                      >
                        {p.sending ? (
                          <Loader size={19} color="currentColor" />
                        ) : (
                          <ArrowUp size={19} />
                        )}
                      </ActionIcon>
                    </div>
                  </div>
                </>
              );
            }}
          </PromptComposer>
        );
      }),
    [
      p.id,
      p.room,
      p.agent,
      p.lead,
      p.sending,
      p.draftConflicts,
      p.syncWorkspaceId,
      p.agent?.accountKey,
      p.agent?.cwd,
      p.agent?.provider,
      stateDir,
      managed,
      canSend,
      threadBlock,
      mobileClient,
      touchInput,
      dragging,
      uploading,
      assets,
      pendingFiles,
      uploadRecovery.error,
      stopShown,
      modelCommandRequest,
      agents,
      visibleMessages,
      before,
      historyVersion,
      addFiles,
      submit,
      actions.getDraft,
      actions.subscribeDraft,
      actions.setDraft,
      actions.notify,
      actions.refresh,
      actions.dismissDraft,
    ],
  ).get();

  return (
    <section
      id="conversation"
      className={p.room ? "agent-conversation" : "ai-conversation"}
    >
      <Modal
        opened={!!branchDraft}
        onClose={() => {
          if (!branching) setBranchDraft(null);
        }}
        title={
          branchDraft?.before
            ? "Edit in a new chat"
            : "Another answer in a new chat"
        }
      >
        <p>
          Opens a new chat with this message as a draft. Files are not included.
        </p>
        <Textarea
          label="Draft for the new chat"
          value={branchDraft?.text || ""}
          autosize
          minRows={3}
          maxRows={10}
          disabled={branching || !!branchDraft?.message.truncated}
          onChange={(event) => {
            const text = event.currentTarget.value;
            setBranchDraft((current) =>
              current ? { ...current, text } : current,
            );
          }}
        />
        {branchDraft?.before && branchDraft.message.truncated && (
          <p>
            The preview is clipped. Create the draft to edit the full message.
          </p>
        )}
        <div className="branch-draft-actions">
          <Button
            variant="subtle"
            disabled={branching}
            onClick={() => setBranchDraft(null)}
          >
            Cancel
          </Button>
          <Button
            loading={branching}
            disabled={!branchDraft?.text.trim()}
            onClick={() => {
              if (branchDraft) void branch(branchDraft.message, branchDraft);
            }}
          >
            Create draft in new chat
          </Button>
        </div>
      </Modal>
      {agent && (detail?.error || threadBlock || capacityRetry) && (
        <ObservedNativeError
          agent={agent}
          detail={detail}
          refresh={actions.refresh}
          planType={planType}
          recovery={
            p.limitsAccounts?.find(
              (account) => account.key === (agent.accountKey || "default"),
            )?.agents?.[agent.id]?.recovery
          }
          openLimits={() => {
            setLimitsOpen(true);
            actions.reloadLimits();
          }}
          refreshLimits={actions.reloadLimits}
          newChat={actions.newConversation}
          chooseChat={actions.chooseConversation}
        />
      )}
      {!p.headerToolsInMenu &&
        headerTools &&
        !p.room &&
        (p.id || managed) &&
        createPortal(
          <details
            className="conversation-header-tools-menu"
            data-compact={compactHeaderTools ? "yes" : "no"}
            open={!compactHeaderTools || toolsExpanded}
            onToggle={(event) => {
              if (compactHeaderTools)
                setToolsExpanded(event.currentTarget.open);
            }}
          >
            <summary
              className="conversation-header-tools-summary"
              aria-label="Conversation tools"
              title="Conversation tools"
            >
              <ListTree size={18} />
              <span className="sr-only">Conversation tools</span>
            </summary>
            <div
              className="conversation-header-tools-content"
              style={
                compactHeaderTools && toolsExpanded && toolsMenuPosition
                  ? {
                      left: toolsMenuPosition.left,
                      top: toolsMenuPosition.top,
                      width: toolsMenuPosition.width,
                    }
                  : compactHeaderTools
                    ? { visibility: "hidden" }
                    : undefined
              }
            >
              {!p.room && p.id && (
                <div className="conversation-prompt-navigation-slot">
                  {promptNavigation}
                </div>
              )}
            </div>
          </details>,
          headerTools,
        )}
      <div
        id="messages"
        ref={scroll}
        onScroll={onHistoryScroll}
        onWheel={(event) => {
          lastHistoryTop.current = scroll.current?.scrollTop ?? 0;
          if (event.deltaY) pageAtEdge(event.deltaY < 0 ? "older" : "newer");
        }}
        onTouchStart={(event) => {
          lastHistoryTop.current = scroll.current?.scrollTop ?? 0;
          touchHistoryY.current = event.touches[0]?.clientY ?? null;
        }}
        onTouchMove={(event) => {
          const y = event.touches[0]?.clientY;
          if (
            y !== undefined &&
            touchHistoryY.current !== null &&
            y !== touchHistoryY.current
          )
            pageAtEdge(y > touchHistoryY.current ? "older" : "newer");
          touchHistoryY.current = y ?? null;
        }}
        onKeyDown={(event) => {
          lastHistoryTop.current = scroll.current?.scrollTop ?? 0;
          if (
            (event.target as HTMLElement).closest(
              "input, textarea, [contenteditable=true]",
            )
          )
            return;
          if (
            ["ArrowUp", "PageUp", "Home"].includes(event.key) ||
            (event.key === " " && event.shiftKey)
          )
            pageAtEdge("older");
          else if (["ArrowDown", "PageDown", "End", " "].includes(event.key))
            pageAtEdge("newer");
        }}
      >
        <div ref={content} className="message-content">
          {p.transcriptHeading && (
            <div className="transcript-heading">{p.transcriptHeading}</div>
          )}
          {removed.hasRemoved && (
            <Button size="compact-xs" onClick={() => removed.restore()}>
              Restore removed messages
            </Button>
          )}
          {notice && <p className="notice">{notice}</p>}
          {historical && (
            <p className="historical-chat-note">
              Earlier part of this chat.
              <Button
                type="button"
                size="compact-xs"
                variant="subtle"
                onClick={returnToLatest}
              >
                Return to latest messages
              </Button>
            </p>
          )}
          <PageIndicator
            store={pageStatus.current}
            id={p.id}
            direction="older"
            retry={() => loadPage("older")}
          />
          {loaded &&
            !visibleMessages.length &&
            !notice &&
            !["running", "starting"].includes(agent?.status || "") &&
            (p.room ? (
              <div className="empty-chat">
                <h2>No messages yet</h2>
                <p>Agent messages will appear here.</p>
              </div>
            ) : (
              <LiveConversationStart agentId={p.id || ""} />
            ))}
          {transcript}
          <PageIndicator
            store={pageStatus.current}
            id={p.id}
            direction="newer"
            retry={() => loadPage("newer")}
          />
          {!p.room && agent && (
            <ObservedSafetyBuffering
              key={`safety:${agent.id}`}
              agent={agent}
              detail={detail}
            />
          )}
          <div className="chat-activity">
            {!detailedActivity &&
              !reasoningVisible &&
              agent &&
              !(
                agent.status === GeneratedApiAgentStatus.queued &&
                nextTurnQueue.length > 0
              ) && (
                <LiveAgentPhase
                  agentId={agent.id}
                  phaseOverride={displayPhase}
                  connection={connection}
                  savedMessagePending={p.offlineSendPending}
                  connectionDetails={p.offlineSendDetails}
                />
              )}
          </div>
          {!p.room && detailedActivity && (
            <LiveAgentPhase
              agentId={agent?.id}
              connection={connection}
              savedMessagePending={p.offlineSendPending}
              connectionDetails={p.offlineSendDetails}
            />
          )}
          <RequestArea
            scope={stateDir}
            refresh={actions.refresh}
            notify={actions.notify}
            onAnswerOpen={() => setFollow(false)}
            onAnswerPosition={revealRequest}
          >
            <Requests
              agentId={p.id}
              familyId={p.room?.rootId}
              includeTeam={!mobileClient}
              showDates={!!p.room}
              excludeIds={inlineRequestIds}
            />
          </RequestArea>
        </div>
      </div>
      {!p.room && (
        <SelectionQuote
          key={`selection:${p.id}`}
          chatId={p.id}
          root={scroll}
          onQuote={quote}
        />
      )}
      <FollowLatest
        subscribe={subscribeFollow}
        getSnapshot={getFollow}
        onClick={returnToLatest}
      />
      {p.room ? (
        <p className="room-footer">
          {p.room.kind === GeneratedApiRoomKind.private
            ? "Private agent chat."
            : "Broadcast to " +
              (p.room.rootId === "all"
                ? "all teams (historical, read only)."
                : "this team.")}
        </p>
      ) : (
        <>
          {managed && (
            <>
              {Boolean(messageQueue.pending) && !messageQueue.busy && (
                <div className="notice queue-notice" role="status">
                  The last queue change needs confirmation.
                  <Button
                    size="compact-xs"
                    onClick={() =>
                      void messageQueue
                        .retry()
                        .catch((error) => actions.notify(errorText(error)))
                    }
                  >
                    Retry queue change
                  </Button>
                </div>
              )}
              <MessageQueue
                loading={messageQueue.loading}
                items={nextTurnQueue}
                scope={queueScope}
                onEdit={messageQueue.edit}
                onCancel={messageQueue.cancel}
                onSendNow={controls.steer ? messageQueue.sendNow : undefined}
                onReorder={(ids) =>
                  messageQueue.reorder(
                    mergeQueueOrder(
                      messageQueue.items.map((item) => item.id),
                      nextTurnQueue.map((item) => item.id),
                      ids,
                    ),
                  )
                }
                canReorder={
                  messageQueue.canReorder &&
                  !nextTurnQueue.some((item) => item.localDelivery)
                }
                refreshing={messageQueue.busy}
                error={messageQueue.error}
              />
            </>
          )}
          {agent && isCyberPolicyRefusal(detail?.error) && (
            <div className="native-policy-composer-warning" role="note">
              <strong>Before you continue</strong>
              <span>
                Continuing this chat sends the same history and can be refused
                again.
              </span>
            </div>
          )}
          {composer}
          <div className="conversation-composer-footer">
            {agent?.source === GeneratedApiAgentSource.managed && (
              <Usage
                key={p.agent?.accountKey || "default"}
                visible={p.visible ?? true}
                agent={agent}
                stateDir={stateDir}
                limits={p.limits}
                accounts={p.limitsAccounts}
                limitsLoading={p.limitsLoading}
                accountLabel={p.limitsAccountLabel}
                reload={actions.reloadLimits}
                opened={limitsOpen}
                onChange={setLimitsOpen}
              />
            )}
          </div>
        </>
      )}
    </section>
  );
}
const ObservedAccountTransferChip = observer(AccountTransferChip);
const ObservedClaudeRepairNotice = observer(ClaudeRepairNotice);
const ObservedSafetyBuffering = observer(SafetyBuffering);
export const ObservedNativeError = observer(NativeError);
const ObservedConversation = observer(ConversationView);
export const LiveConversation = observer(function LiveConversation({
  ...props
}: Parameters<typeof ConversationView>[0] & {}) {
  const store = useServerStore();
  const state = resourceData(store.state().current);
  // A confirmed create can precede the authoritative collection. Keep that
  // narrow fallback until the stream supplies its canonical model.
  const agent = state?.agents.get(props.id ?? "") ?? props.agent;
  const lead = state?.agents.get(props.lead?.id ?? "") ?? props.lead;
  const detail = agent
    ? resourceData(store.agentDetail(resourceAgentId(agent.id)).current)
    : undefined;
  return state ? (
    <ObservedConversation
      {...props}
      agent={agent}
      lead={lead}
      detail={detail}
    />
  ) : null;
});
export default ObservedConversation;

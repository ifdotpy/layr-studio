import { resourceData } from "../store/runtime.ts";
import { useLimitRefresh } from "../usage/useLimitRefresh.ts";
import {
  TranscriptProvider,
  useConversationActions,
} from "./conversation/navigation.ts";
import { observer } from "mobx-react-lite";
import { serverStatus } from "../messageSendStatus";
import {
  MessageSendStatus,
  AgentSource as GeneratedApiAgentSource,
  AgentStatus as GeneratedApiAgentStatus,
  RoomKind as GeneratedApiRoomKind,
} from "../generated/api.ts";
import { randomUUID } from "../uuid";
import { codePointsExceed, sendsTextFile } from "../commands";
import { ProviderKind, providerSupports } from "../generated/providerKind.ts";
import type { ReactNode } from "react";
import { IdleCompactionNotice } from "./IdleCompactionNotice";
import { resolveLayoutLength } from "../layout/deriveWorkspaceLayout";
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
  useServerUploads,
} from "../servers/ServerServicesContext";
import { useServerActivity } from "../servers/activity";
import {
  computed,
  compareShallow,
  makeAutoObservable,
  observableRef,
  runInAction,
} from "mobx";
import AgentAvatar from "./agents/AgentAvatar";
import MessageQueue, {
  type LocalQueueItem,
  type QueueItem,
} from "./MessageQueue";
import { useMessageQueue } from "./useMessageQueue";
import { useVisibleChatResult } from "./useChatReadState";
import {
  NativeError,
  NativeNotice,
  isNonBlockingWarning,
} from "./NativeNotice";
import { ActionIcon, Button, Loader, Modal, Textarea } from "@mantine/core";
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
import { transcriptMessages } from "../conversation/messageView";
import DraftVersions from "./DraftVersions";
import { useDisplayPhase } from "./useDisplayPhase";
import {
  deliveryLabel,
  messageRenderKey,
  explicitQueue,
  dispatchedMessage,
  mergeQueueOrder,
} from "./message-delivery/messageDelivery";

import { useConversationScroll } from "./useConversationScroll";
import { useAttachmentDrafts } from "./useAttachmentDrafts";
import { useUploadRecovery } from "./useUploadRecovery";
import { type KnownApiFields, type Message } from "../types";
import Usage from "./Usage";
import PromptNavigator from "./PromptNavigator";
import { usePromptRecall } from "./usePromptRecall";
import PromptComposer from "./prompt-composer/PromptComposer";
import PromptInput from "./prompt-composer/PromptInput";
import { SelectedTitle } from "../app/conversation/SelectedTitle.tsx";
import { reportPromptComposerRender } from "./prompt-composer/renderProbe";
import Requests, { RequestArea, RequestCard } from "./questions/Requests";
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
const PageIndicator = observer(function PageIndicator({
  store,
  id,
  direction,
  retry,
}: {
  store: { value: PageStatus };
  id: string | null | undefined;
  direction: "older" | "newer";
  retry: () => void;
}) {
  const status = store.value;
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
});

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

const emptyAttachments: Attachment[] = [];
type BranchDraft = { message: Message; before: boolean; text: string };

export function ConversationView() {
  const actions = useConversationActions();
  const { selected, navigation, delivery, startup, layout, read, scope } =
    actions;
  const {
    selectedId: opened,
    selectedAgent: agent,
    selectedRoom: room,
    selectedLead: lead,
    selectedLegacyChat: legacy,
    selectedJumpTarget: jumpTarget,
  } = navigation;
  const { detail } = selected;
  const { services } = scope,
    drafts = services.drafts;
  const { notices, creation, sidebar } = actions;
  const stateDir = startup.creationScope || "",
    agents = selected.team;
  const { removeSendingMessage } = useSendingMessageRemoval();
  const { queueUploads } = useServerUploads();
  const api = useServerApi();
  const { get } = api;
  const { changeOutbox } = useServerOutbox();

  const sending = delivery.isSending;
  reportPromptComposerRender("conversation");
  const [limitsOpen, setLimitsOpen] = useState(false);
  const [limitsOpenRequest, setLimitsOpenRequest] = useState(0);
  const limitRefresh = useLimitRefresh(get);
  const accountKey = agent?.accountKey || "default";
  const planType = selected.planType;
  const mobileClient = layout.isMobile;
  // A touch keyboard's Enter adds a new line at every width; the button sends.
  const touchInput = usePrimaryTouch();
  const kind = room ? "room" : legacy ? "legacy" : "agent";
  const compactHeaderTools = layout.usesCompactConversationTools;
  const [headerTools, setHeaderTools] = useState<HTMLElement | null>(null);
  const [toolsExpanded, setToolsExpanded] = useState(false);
  const [toolsMenuPosition, setToolsMenuPosition] = useState<{
    left: number;
    top: number;
    width: number;
  } | null>(null);
  useLayoutEffect(() => {
    setHeaderTools(
      layout.usesMenuHeaderTools
        ? null
        : document.getElementById("conversation-header-tools"),
    );
  }, [layout.usesMenuHeaderTools]);
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
  const historySession = selected.history;
  const {
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
  const outgoing = delivery.selectedOutgoing;
  const removed = selected.removed;
  const queueScope = `${stateDir}:${startup.workspaceId || ""}:${opened}`;
  const messageQueue = useMessageQueue({
    id: opened,
    enabled: agent?.source === GeneratedApiAgentSource.managed,
    scope: queueScope,
    workspaceId: startup.workspaceId,
    observed: delivery.observeSends,
    edited: delivery.editSend,
    refresh: actions.refresh,
  });
  const deliveryTranscript = selected.deliveryTranscript;
  const visibleMessages = selected.visibleMessages;
  const historyAgentId = opened;
  const promptRecall = usePromptRecall(
    `${stateDir}:${kind}:${opened}:${historyVersion}`,
    room ? [] : visibleMessages,
    () => drafts.getDraft(opened || "new"),
    delivery.editDraft,
    agent?.source === GeneratedApiAgentSource.managed &&
      kind === "agent" &&
      historyAgentId
      ? {
          before: before ? String(before) : null,
          load: async (cursor) => {
            const page = await scope.store.reads
              .transcriptPage(api, {
                id: historyAgentId,
                before: cursor,
                limit: 500,
              })
              .read();
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
          onError: (error) => notices.notify(errorText(error)),
        }
      : undefined,
  );
  const observed = deliveryTranscript.observed.join(",");
  useEffect(() => {
    if (observed) delivery.observeSends?.(observed.split(","));
  }, [observed, delivery.observeSends]);
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
    `${stateDir}:${kind}:${opened}`,
    loaded,
    !after,
    `studio-turns:${stateDir}:${opened}`,
  );
  const pageStatus = useRef(
    makeAutoObservable(
      {
        value: null as PageStatus,
        navigation,
        set(value: PageStatus) {
          this.value = value;
        },
      },
      { navigation: false },
      { autoBind: true },
    ),
  ).current;
  pageStatus.navigation = navigation;
  const paging = useRef<{ id: string | null | undefined } | null>(null);
  const loadPage = (direction: "older" | "newer") => {
    if (paging.current?.id === opened) return;
    const request = { id: opened };
    paging.current = request;
    pageStatus.set({ id: opened, direction, failed: false });
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
        if (paging.current === request) pageStatus.set(null);
      })
      .catch((error) => {
        if (
          paging.current === request &&
          pageStatus.navigation?.selectedId === request.id
        ) {
          pageStatus.set({ id: opened, direction, failed: true });
          notices.notify(errorText(error));
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
      paging.current?.id === opened ||
      (pageStatus.value?.id === opened && pageStatus.value.failed)
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

  const navigateToMessage = async (id: string) => {
    const root = scroll.current;
    if (!root) return;
    resetHistoryWindowAnchor(root);
    const chat = opened;
    const attempt = ++navigationAttempt.current;
    setFollow(false);
    setPageAnchor(getAnchorId());
    const available = await ensureMessage(id);
    if (
      pageStatus.navigation?.selectedId !== chat ||
      navigationAttempt.current !== attempt
    )
      return;
    if (!available)
      throw new Error("This message is unavailable in this conversation.");
    for (let frame = 0; frame < 20; frame++) {
      if (
        scroll.current !== root ||
        pageStatus.navigation?.selectedId !== chat ||
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
          control.setHighlighted(target.dataset.message || null);
          return;
        }
      }
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => resolve()),
      );
    }
    throw new Error("The message could not be displayed. Search again.");
  };
  const returnToLatest = () => {
    if (scroll.current) resetHistoryWindowAnchor(scroll.current);
    const attempt = ++navigationAttempt.current;
    const chat = opened;
    control.setHighlighted(null);
    void showLatest()
      .then(() => {
        if (
          pageStatus.navigation?.selectedId === chat &&
          navigationAttempt.current === attempt
        )
          setFollow(true);
      })
      .catch((error) => notices.notify(errorText(error)));
  };
  useEffect(() => {
    if (!jumpTarget || handledJump.current === jumpTarget.requestId) return;
    const target = jumpTarget;
    handledJump.current = target.requestId;
    void navigateToMessage(target.messageId).catch((error) =>
      notices.notify(errorText(error)),
    );
  }, [jumpTarget?.requestId, opened]);
  const attachmentKey = `codex-agent-attachments:${stateDir}`;
  const [attachments, setAttachments] = useAttachmentDrafts(
    attachmentKey,
    notices.notify,
  );
  const uploadRecovery = useUploadRecovery(
    stateDir,
    startup.workspaceId || "",
    setAttachments,
  );
  const pendingCommands = usePendingCommands();
  const [operations] = useState(() => ({
    latestSend: {} as Record<string, symbol>,
    branchLock: false,
    branchRequests: {} as Record<string, string>,
    uploadLock: false,
  }));
  const control = useMemo(() => {
    const control = {
      pendingUploads: uploadRecovery.pending,
      navigateToMessage: async (_id: string) => {},
      returnToLatest: () => {},
      attachments,
      pendingCommands: [] as ReturnType<typeof usePendingCommands>,
      addingFiles: false,
      stopping: false,
      branching: false,
      editLoading: null as string | null,
      highlighted: null as string | null,
      dragging: false,
      removingSending: false,
      editAttempt: 0,
      modelCommandRequest: 0,
      branchDraft: null as BranchDraft | null,
      get agent() {
        return actions.navigation.selectedAgent;
      },
      get managed() {
        return this.agent?.source === GeneratedApiAgentSource.managed;
      },
      get visibleMessages() {
        return actions.selected.visibleMessages;
      },
      get assets() {
        return this.attachments[opened || ""] || emptyAttachments;
      },
      get pendingFiles() {
        return this.pendingUploads.filter((row) => row.agent === opened);
      },
      get uploading() {
        return this.addingFiles || this.pendingFiles.length > 0;
      },
      get stopShown() {
        return (
          this.stopping ||
          this.pendingCommands.some(
            ({ kind, target }) => kind === "stop" && target === opened,
          )
        );
      },
      matchesQueue(item: Message, entry: QueueItem) {
        return (
          item.role === "user" &&
          (entry.id === item.clientMessageId ||
            entry.id === item.id ||
            opened + ":" + entry.id === item.id)
        );
      },
      get pendingQueueEntries(): QueueItem[] {
        const { visibleMessages } = this;

        return [
          ...messageQueue.items,
          ...visibleMessages
            .filter(
              (item) =>
                item.role === "user" &&
                explicitQueue(item) &&
                !dispatchedMessage(item) &&
                !messageQueue.items.some((row) => this.matchesQueue(item, row)),
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
          (entry: QueueItem) =>
            (entry.localDelivery ||
              entry.status === MessageSendStatus.queued) &&
            !visibleMessages.some(
              (item) =>
                dispatchedMessage(item) && this.matchesQueue(item, entry),
            ),
        );
      },
      isNextTurn(entry: QueueItem) {
        return (
          (("delivery" in entry ? entry.delivery : undefined) ||
            entry.requestedDelivery) === DeliveryMoment.NextTurn
        );
      },
      get nextTurnQueue() {
        return this.pendingQueueEntries.filter(this.isNextTurn);
      },
      get currentTurnQueue() {
        return this.pendingQueueEntries.filter(
          (entry) => !this.isNextTurn(entry),
        );
      },
      get lastAssistantByTurn() {
        const uncertain = new Set(
          this.visibleMessages
            .filter((m) => m.turnStatus === "uncertain")
            .map((m) => m.turnId),
        );
        const last = new Map<string, string>();
        for (const m of this.visibleMessages)
          if (
            m.role === "assistant" &&
            m.turnId &&
            !uncertain.has(m.turnId) &&
            !isEmptyAssistantMessage(m)
          )
            last.set(m.turnId, m.id);
        return last;
      },
      get renderedMessages(): Message[] {
        const { visibleMessages, currentTurnQueue, nextTurnQueue } = this;

        const removed = actions.selected.removed;
        return [
          ...visibleMessages.filter(
            (item) =>
              !nextTurnQueue.some((entry) => this.matchesQueue(item, entry)),
          ),
          ...currentTurnQueue
            .filter(
              (entry) =>
                !visibleMessages.some((item) => this.matchesQueue(item, entry)),
            )
            .map((entry): Message => ({
              id: entry.id,
              clientMessageId: entry.id,
              role: "user",
              text: entry.text,
              ...(entry.assets ? { assets: entry.assets } : {}),
              localDelivery: true,
              deliveryStatus: entry.localDelivery
                ? entry.deliveryStatus
                : { source: "server", status: entry.status },
            })),
        ].filter((message) => !removed.hidden(message));
      },
      get inlineRequests() {
        const { requestArea } = selected;
        return transcriptRequests(
          this.managed && !room ? this.renderedMessages : [],
          requestArea.requests.requests,
          requestArea.cards,
        );
      },
      setUploading(value: boolean) {
        this.addingFiles = value;
      },
      setEditLoading(value: string | null) {
        this.editLoading = value;
      },
      setBranching(value: boolean) {
        this.branching = value;
      },
      setHighlighted(value: string | null) {
        this.highlighted = value;
      },
      setDragging(value: boolean) {
        this.dragging = value;
      },
      setRemovingSending(value: boolean) {
        this.removingSending = value;
      },
      setBranchDraft(value: React.SetStateAction<BranchDraft | null>) {
        this.branchDraft =
          typeof value === "function" ? value(this.branchDraft) : value;
      },
      stop() {
        if (this.stopShown || !opened) return;
        this.stopping = true;
        void api.command
          .stop(opened, false)
          .then(actions.refresh)
          .catch((error) => actions.notices.notify(errorText(error)))
          .finally(() =>
            runInAction(() => {
              this.stopping = false;
            }),
          );
      },
      jumpToPrompt(id: string) {
        void this.navigateToMessage(id).catch((error) =>
          notices.notify(errorText(error)),
        );
      },
      async addFiles(files: globalThis.File[]) {
        const { assets, pendingFiles } = this;

        if (!opened || !this.managed)
          throw new Error("Select a managed chat before attaching files.");
        if (operations.uploadLock)
          throw new Error(
            "Wait for the selected files to be saved, then retry.",
          );
        if (!files.length) throw new Error("Select at least one file.");
        if (
          files.length + assets.length + pendingFiles.length >
          MAX_TURN_ATTACHMENTS
        )
          throw new Error(`Attach at most ${MAX_TURN_ATTACHMENTS} files.`);
        const id = opened;
        operations.uploadLock = true;
        this.setUploading(true);
        try {
          uploadRecovery.include(
            await queueUploads(stateDir, id, files, startup.workspaceId || ""),
          );
          uploadRecovery.retry();
        } finally {
          operations.uploadLock = false;
          this.setUploading(false);
        }
      },
      async submit(
        deliveryMoment: DeliveryMoment = DeliveryMoment.CurrentTurn,
        asTextFile: boolean = false,
      ) {
        const { agent, assets } = this;

        const draft = drafts.getDraft(opened || "new");
        const draftTooLong = codePointsExceed(draft, MAX_DRAFT_TEXT_CODEPOINTS);
        const modelCommand = modelCommandPattern.test(draft.trim());
        const exactModelCommand = exactModelCommandPattern.test(draft.trim());
        if (!sendsTextFile(draft, asTextFile) && modelCommand) {
          if (!exactModelCommand) {
            notices.notify("Use /model without arguments to choose a model.");
          } else if (!this.managed || !agent) {
            notices.notify("Select a managed chat to choose a model.");
          } else {
            promptRecall.reset();
            input.current?.focus({ preventScroll: true });
            delivery.editDraft("");
            this.modelCommandRequest++;
          }
          return;
        }
        if (
          delivery.isSending ||
          nativeThreadError(this.agent, selected.detail) ||
          this.uploading ||
          draftTooLong ||
          (!draft.trim() && !assets.length)
        )
          return;
        const attempt = Symbol();
        const id = opened || "";
        operations.latestSend[id] = attempt;
        // Sending expresses a new scroll intent. Streaming still respects a later
        // manual scroll away from the bottom.
        this.returnToLatest();
        input.current?.focus({ preventScroll: true });
        const sent = new Set(assets.map((asset) => asset.id));
        try {
          await delivery.send({
            ...(asTextFile ? { asTextFile: true } : {}),
            delivery: deliveryMoment,
            assets: assets.map((asset) => asset.id),
            attachments: assets.map(({ preview: _preview, ...asset }) => asset),
            onPersist: async () => {
              await setAttachments((current) => ({
                ...current,
                [id]: (current[id] || []).filter(
                  (asset) => !sent.has(asset.id),
                ),
              }));
            },
          });
          if (id && this.managed)
            void messageQueue
              .reload()
              .catch((error) => notices.notify(errorText(error)));
        } catch {
          setAttachments((current) =>
            operations.latestSend[id] !== attempt
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
        }
      },
      async editMessage(message: Message) {
        const get = api.get;

        const attempt = ++this.editAttempt;
        const chat = opened;
        this.setEditLoading(message.id);
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
          if (
            actions.navigation.selectedId === chat &&
            attempt === this.editAttempt
          )
            this.setBranchDraft({
              message: original,
              before: true,
              text: original.text,
            });
        } catch (error) {
          if (
            actions.navigation.selectedId === chat &&
            attempt === this.editAttempt
          )
            notices.notify(errorText(error));
        } finally {
          if (attempt === this.editAttempt) this.setEditLoading(null);
        }
      },
      async branch(
        message: Message,
        draft?: { before: boolean; text: string },
      ) {
        const { visibleMessages } = this;
        const post = api.post;

        if (
          visibleMessages.some(
            (item) =>
              item.turnId === message.turnId && item.turnStatus === "uncertain",
          )
        ) {
          notices.notify(
            "The turn outcome is unconfirmed. Review the saved evidence before you continue.",
          );
          return;
        }
        if (operations.branchLock) return;
        const agentId = opened;
        if (!agentId) return;
        operations.branchLock = true;
        const sourceId = draft?.before
          ? message.id
          : message.sourceId || message.id;
        const key = `${agentId}:${sourceId}:${draft?.before ? "before" : "after"}`;
        operations.branchRequests[key] ||= randomUUID();
        this.setBranching(true);
        try {
          const response = await post("/api/branch", {
            agent: agentId,
            message_id: sourceId,
            id: operations.branchRequests[key],
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
            delivery.editDraft(
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
          actions.open(branchId);
          this.setBranchDraft(null);
          delete operations.branchRequests[key];
        } catch (error) {
          notices.notify(errorText(error));
        } finally {
          operations.branchLock = false;
          this.setBranching(false);
        }
      },
      async copy(text: string) {
        try {
          await copyText(text);
          notices.notify("Copied.");
        } catch {
          notices.notify("Clipboard access failed.");
        }
      },
      quote(text: string) {
        const quoted = text
          .split(/\r?\n/)
          .map((line) => `> ${line}`)
          .join("\n");
        const draft = drafts.getDraft(opened || "new");
        const separator =
          !draft || draft.endsWith("\n\n")
            ? ""
            : draft.endsWith("\n")
              ? "\n"
              : "\n\n";
        delivery.editDraft(`${draft}${separator}${quoted}\n\n`);
        input.current?.focus();
      },
      async removeSending(messages: Message[]) {
        if (this.removingSending || !opened) return;
        this.setRemovingSending(true);
        try {
          for (const message of messages)
            await removeSendingMessage(
              { stateDir: stateDir, workspaceId: startup.workspaceId, kind },
              { chat: opened, message },
            );
          void messageQueue.reload().catch(() => {});
          notices.notify(
            `Removed ${messages.length} messages. Work already sent continues.`,
          );
        } catch (error) {
          notices.notify(errorText(error));
        } finally {
          this.setRemovingSending(false);
        }
      },
      restoreDraft(m: Message) {
        const draft = drafts.getDraft(opened || "new");
        const separator =
          draft.trim() && draft.trim() !== m.text.trim() ? "\n\n" : "";
        delivery.editDraft(separator ? draft + separator + m.text : m.text);
        setAttachments((current) => ({
          ...current,
          [opened || ""]: [
            ...new Map(
              [...(current[opened || ""] || []), ...(m.assets || [])].map(
                (asset: Attachment) => [asset.id, asset],
              ),
            ).values(),
          ],
        }));
        input.current?.focus({ preventScroll: true });
      },
      async removeMessage(m: Message) {
        try {
          if (!m.clientMessageId) return;
          await changeOutbox(m.clientMessageId, "cancel");
          delivery.observeSends?.([m.clientMessageId]);
          notices.notify(
            "Message removed from this device. Delivery was not cancelled.",
          );
        } catch (error) {
          notices.notify(`Could not remove this message: ${errorText(error)}`);
        }
      },
    };
    return makeAutoObservable(
      control,
      {
        navigateToMessage: false,
        returnToLatest: false,
        pendingUploads: observableRef,
        attachments: observableRef,
        pendingCommands: observableRef,
        branchDraft: observableRef,
        lastAssistantByTurn: computed({ equals: compareShallow }),
        inlineRequests: computed({ equals: compareShallow }),
      },
      { autoBind: true },
    );
  }, [actions, opened, stateDir, kind, messageQueue]);
  useLayoutEffect(() => {
    runInAction(() => {
      control.attachments = attachments;
      control.pendingUploads = uploadRecovery.pending;
      control.pendingCommands = pendingCommands;
      control.navigateToMessage = navigateToMessage;
      control.returnToLatest = returnToLatest;
    });
  });

  useServerActivity(control.addingFiles || uploadRecovery.pending.length > 0);
  const controls = {
    steer: providerSupports(agent?.provider, "steer"),
    fork: providerSupports(agent?.provider, "fork"),
  };
  useVisibleChatResult(
    scroll,
    agent,
    visibleMessages,
    loaded,
    read.observeRead,
    startup.workspaceId,
    !historical && !after,
  );
  const threadBlock = nativeThreadError(agent, detail);
  const capacityRetry = agent ? currentCapacityRetry(agent) : null;
  const displayPhase = useDisplayPhase(
    opened,
    detail?.activity?.phase || "thinking",
  );
  const detailedActivity =
    !!threadBlock ||
    capacityRetry?.status === "scheduled" ||
    !!connection?.message ||
    !!delivery.offlinePending ||
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
  const managed = agent?.source === GeneratedApiAgentSource.managed;
  const first =
    room?.members?.[0] || visibleMessages.find((m) => m.sender)?.sender;
  const canSend = selected.canSend;
  const questionCards = useCallback(
    (item: Message) => selected.requestArea.cards(item),
    [selected],
  );
  const inlineRequestIds = new Set(
    [...control.inlineRequests.values()].flatMap((request) =>
      request ? [request.id] : [],
    ),
  );
  const renderMessage = useCallback(
    (m: Message) => {
      reportPromptComposerRender("message", m.id);
      return renderMessageArticle(m);
    },
    [
      control,
      delivery,
      room,
      opened,
      first,
      managed,
      agent,
      controls.fork,
      outgoing,
      stateDir,
      agents,
      notices.notify,
      delivery.observeSends,
    ],
  );
  const renderMessageArticle = (m: Message) => (
    <article
      key={messageRenderKey(m)}
      data-message={m.id}
      data-source-message={m.sourceId}
      data-found={control.highlighted === m.id || undefined}
      className={`message ${m.role === "user" ? "user" : "assistant"} ${room ? "bubble " + (m.sender !== first ? "outgoing" : "incoming") : ""}`}
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
                disabled={control.removingSending}
                onClick={() => void control.removeSending([m])}
              >
                <Trash2 size={14} />
              </ActionIcon>
            )}
          </span>
        </SettledDelivery>
      )}
      {/* The main chat has two speakers; only team rooms name each sender. */}
      {room && (
        <span className="chat-message-author">
          <AgentAvatar
            id={m.sender || agent?.id || opened || "agent"}
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
                    onClick={() => control.removeMessage(m)}
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
            room
              ? agents.find((item) => item.id === m.sender)?.id
              : managed
                ? agent?.id
                : undefined
          }
        />
      )}
      {Array.isArray(m.assets) && (
        <MessageAttachments assets={m.assets} notify={notices.notify} />
      )}
      {m.localDelivery && (
        <OutboxControls
          entry={outgoing?.find((entry) => entry.id === m.clientMessageId)}
          onResolved={(id) => delivery.observeSends?.([id])}
        />
      )}
      {m.deliveryError &&
        !(
          delivery.offlinePending &&
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
                onClick={() => control.restoreDraft(m)}
              >
                Restore draft
              </Button>
            )}
          </div>
        )}
      {m.truncated && <p className="notice">This message is clipped.</p>}
      {m.fullTextBlob && <FullOutput blob={m.fullTextBlob} />}
      {(room || m.role === "user") && (
        <MessageDate at={m.at ?? m.created ?? m.timestamp} />
      )}
      <MessageActions
        hidden={!!m.streaming}
        info={
          m.role === "assistant"
            ? {
                message: m,
                agentId: !room && managed ? agent?.id : undefined,
                stateDir: stateDir,
                rootId:
                  !room && managed ? agent?.rootId || agent?.id : undefined,
                accounts:
                  resourceData(
                    scope.store.state().current,
                  )?.accountsSummary.get("current")?.accounts || [],
                onCopy: (value) => void control.copy(value),
              }
            : undefined
        }
        onCopy={() => void control.copy(m.text)}
        onQuote={
          !room && !explicitQueue(m)
            ? () => {
                const excerpt = selectedExcerpt(scroll.current);
                control.quote(
                  excerpt?.messageId === m.id ? excerpt.text : m.text,
                );
                window.getSelection()?.removeAllRanges();
              }
            : undefined
        }
        onEdit={
          !room &&
          !explicitQueue(m) &&
          managed &&
          m.role === "user" &&
          m.turnId &&
          m.turnStatus !== "uncertain" &&
          (!m.deliveryStatus ||
            serverStatus(m.deliveryStatus) === MessageSendStatus.delivered)
            ? () => void control.editMessage(m)
            : undefined
        }
        editLoading={control.editLoading === m.id}
        editDisabled={
          control.branching ||
          !!control.editLoading ||
          m.turnId === agent?.turnId
        }
        onBranch={
          controls.fork &&
          !room &&
          !explicitQueue(m) &&
          managed &&
          m.role === "assistant" &&
          m.turnId &&
          m.turnId !== agent?.turnId &&
          m.turnStatus !== "uncertain" &&
          control.lastAssistantByTurn.get(m.turnId) === m.id
            ? () => void control.branch(m)
            : undefined
        }
        branchDisabled={m.turnId === agent?.turnId && !!agent?.inFlight}
        onAnotherAnswer={
          !room &&
          !explicitQueue(m) &&
          managed &&
          m.role === "assistant" &&
          m.turnId &&
          m.turnId !== agent?.turnId &&
          control.lastAssistantByTurn.get(m.turnId) === m.id
            ? () =>
                control.setBranchDraft({
                  message: m,
                  before: false,
                  text: "Give another answer to my previous request. Use the existing results. Do not run tools or commands unless I explicitly ask.",
                })
            : undefined
        }
        anotherAnswerDisabled={control.branching}
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
  const inlineRequests = control.inlineRequests;
  const onAnswerOpen = useCallback(() => setFollow(false), [setFollow]);
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
              notify={notices.notify}
              onAnswerOpen={onAnswerOpen}
              onAnswerPosition={revealRequest}
            >
              <RequestCard
                request={request}
                messageId={messageId}
                showDates={!!room}
              />
            </RequestArea>
          ),
        ]),
      ),
    [
      inlineRequests,
      stateDir,
      actions.refresh,
      notices.notify,
      onAnswerOpen,
      revealRequest,
      room,
    ],
  );
  const transcriptActions = useMemo(
    () => ({
      renderMessage: renderTranscriptMessage,
      questionCards,
      requestCards,
      jumpToMessage: control.jumpToPrompt,
    }),
    [renderTranscriptMessage, questionCards, requestCards, control],
  );
  const transcript = (
    <TranscriptProvider value={transcriptActions}>
      {managed && !room && agent?.provider === ProviderKind.Claude && (
        <ObservedClaudeRepairNotice agent={agent} detail={detail} />
      )}
      <TurnHistory
        key={stateDir + ":" + opened}
        history={historySession}
        items={control.renderedMessages}
        scrollContainer={scroll}
        rememberScroll={remember}
        getFollow={getFollow}
        followTail={!after}
        hasRecentScrollInput={hasRecentScrollInput}
        agent={managed && !room ? agent : undefined}
        currentTurn={agent?.turnId || undefined}
        enabled={managed && !room}
        storageKey={"studio-turns:" + stateDir + ":" + opened}
        agentId={managed ? agent?.id : undefined}
      />
    </TranscriptProvider>
  );
  const promptNavigation =
    !room && opened ? (
      <PromptNavigator
        compact
        inMenu={layout.usesMenuHeaderTools}
        key={opened}
        messages={visibleMessages}
        loadOlder={
          before
            ? () => {
                setFollow(false);
                setPageAnchor(getAnchorId());
                void older().catch((e) => notices.notify(errorText(e)));
              }
            : undefined
        }
        loadingOlder={pageLoading}
        agentId={managed ? opened || undefined : undefined}
        container={scroll}
        storageKey={`studio-prompt-bookmarks:${stateDir}:${opened}`}
        jump={control.jumpToPrompt}
      />
    ) : null;
  useLayoutEffect(() => {
    selected.railTools.publish?.(
      layout.usesMenuHeaderTools ? promptNavigation : null,
    );
  }, [
    layout.usesMenuHeaderTools,
    selected.railTools.publish,
    promptNavigation,
  ]);
  useLayoutEffect(
    () => () => selected.railTools.publish?.(null),
    [selected.railTools.publish],
  );
  // The composer renders again for every keystroke. Its parts that do not
  // read the draft are built here, with the chat, so React reuses them.

  const composer = (() => {
    const canStopAgent =
      !!agent &&
      (agent.status === GeneratedApiAgentStatus.running ||
        agent.status === GeneratedApiAgentStatus.starting ||
        agent.status === GeneratedApiAgentStatus.approval);
    const executionControls = managed && agent && (
      <ConversationSettings
        agent={agent}
        lead={lead}
        openRequest={control.modelCommandRequest}
      />
    );
    const attachTool = managed && (
      <ComposerAttachments
        notify={notices.notify}
        assets={control.assets}
        uploading={control.uploading}
        disabled={!canSend || sending}
        add={control.addFiles}
        remove={(id) => {
          const agentId = id || "";
          void uploadRecovery
            .remove(id, () => {
              setAttachments((current) => ({
                ...current,
                [agentId]: (current[agentId] || []).filter(
                  (asset) => asset.id !== id,
                ),
              }));
            })
            .catch((error) => notices.notify(errorText(error)));
        }}
      />
    );
    const voiceTool = managed &&
      agent?.isLead &&
      agent?.provider === ProviderKind.Codex &&
      opened &&
      !threadBlock && (
        <RealtimeVoice
          key={`voice:${opened}`}
          agentId={opened}
          notify={notices.notify}
        />
      );
    const transferChip = managed && agent?.isLead && (
      <ObservedAccountTransferChip transfer={agent.accountTransferProgress} />
    );
    const draftVersions = (
      <div className="composer-draft-control">
        <DraftVersions
          key={`drafts:${opened || "new"}`}
          versions={delivery.selectedDraftConflicts || []}
          useVersion={(version, mode) =>
            delivery.editDraft((current) =>
              mode === "append" && current
                ? `${current}\n\n${version.text}`
                : version.text,
            )
          }
          dismiss={(version) => drafts.dismissDraft?.(version)}
        />
      </div>
    );
    const pendingUploads = managed && (
      <>
        {!!control.pendingFiles.length && (
          <div className="attachment-list" role="status">
            {control.pendingFiles.map((file) => (
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
                      .catch((error) => notices.notify(errorText(error)));
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
    const dictation = managed && opened && (
      <Dictation
        key={`${stateDir}:${opened}`}
        chatId={`${stateDir}:${opened}`}
        disabled={!canSend || sending}
        onInsert={(text) =>
          delivery.editDraft(
            (current) =>
              current + (current && !current.endsWith("\n") ? "\n" : "") + text,
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
            disabled={control.stopShown || !canStopAgent}
            aria-busy={control.stopShown}
            onClick={control.stop}
          >
            {control.stopShown ? (
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
        session={opened || "new"}
        getDraft={drafts.getDraft}
        subscribeDraft={drafts.subscribeDraft}
        className={control.dragging ? "attachment-drop" : ""}
        onDragOver={(event) => {
          if (managed) {
            event.preventDefault();
            control.setDragging(true);
          }
        }}
        onDragLeave={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node))
            control.setDragging(false);
        }}
        onDrop={(event) => {
          event.preventDefault();
          control.setDragging(false);
          void control
            .addFiles(Array.from(event.dataTransfer.files))
            .catch((error) => notices.notify(errorText(error)));
        }}
        onSubmit={(e) => {
          e.preventDefault();
          void control.submit();
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
          const exactModelCommand = exactModelCommandPattern.test(draft.trim());
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
                  sending ||
                  control.uploading ||
                  draftTooLong ||
                  (!draft.trim() && !control.assets.length)
                }
                onClick={() => void control.submit(DeliveryMoment.NextTurn)}
              >
                <ListEnd size={18} />
              </ActionIcon>
            ) : null;
          return (
            <>
              {transferChip}
              <PromptInput
                value={draft}
                draftResetVersion={
                  delivery.draftResetVersions.get(opened || "new") ?? 0
                }
                session={opened || "new"}
                getDraft={drafts.getDraft}
                setDraft={delivery.editDraft}
                input={input}
                mobile={mobileClient || touchInput}
                managed={!!managed}
                canSend={canSend}
                blocked={!!threadBlock}
                sending={sending}
                uploading={control.uploading}
                draftTooLong={draftTooLong}
                modelCommand={modelCommand}
                hasAttachments={!!control.assets.length}
                onChange={(value) => {
                  promptRecall.reset();
                  delivery.editDraft(value);
                }}
                onPasteFiles={(files) =>
                  void control
                    .addFiles(files)
                    .catch((error) => notices.notify(errorText(error)))
                }
                onSend={() => void control.submit()}
                onQueue={() => void control.submit(DeliveryMoment.NextTurn)}
                onRecallKeyDown={promptRecall.onKeyDown}
                skillCatalog={{
                  enabled:
                    !!managed && !room && !!opened && opened === agent?.id,
                  agentId: opened || "",
                  workspace: startup.workspaceId || stateDir,
                  account: agent?.accountKey || "default",
                  cwd: agent?.cwd || "",
                  provider: String(agent?.provider || ""),
                }}
              />
              {exactModelCommand && (
                <Button
                  type="button"
                  variant="subtle"
                  size="compact-sm"
                  aria-label="Choose model with /model"
                  onClick={() => void control.submit()}
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
                        sending ||
                        control.uploading ||
                        draftTooLong ||
                        control.assets.length >= MAX_TURN_ATTACHMENTS
                      }
                      onClick={() =>
                        void control.submit(DeliveryMoment.CurrentTurn, true)
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
                  {sending ? "Sending…" : ""}
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
                      sending ||
                      control.uploading ||
                      draftTooLong ||
                      (!draft.trim() && !control.assets.length)
                    }
                    aria-label="Send message"
                    title={
                      managed ? "Send after tool calls (Enter)" : "Send message"
                    }
                  >
                    {sending ? (
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
  })();

  return (
    <section
      id="conversation"
      className={room ? "agent-conversation" : "ai-conversation"}
    >
      <Modal
        opened={!!control.branchDraft}
        onClose={() => {
          if (!control.branching) control.setBranchDraft(null);
        }}
        title={
          control.branchDraft?.before
            ? "Edit in a new chat"
            : "Another answer in a new chat"
        }
      >
        <p>
          Opens a new chat with this message as a draft. Files are not included.
        </p>
        <Textarea
          label="Draft for the new chat"
          value={control.branchDraft?.text || ""}
          autosize
          minRows={3}
          maxRows={10}
          disabled={
            control.branching || !!control.branchDraft?.message.truncated
          }
          onChange={(event) => {
            const text = event.currentTarget.value;
            control.setBranchDraft((current) =>
              current ? { ...current, text } : current,
            );
          }}
        />
        {control.branchDraft?.before &&
          control.branchDraft.message.truncated && (
            <p>
              The preview is clipped. Create the draft to edit the full message.
            </p>
          )}
        <div className="branch-draft-actions">
          <Button
            variant="subtle"
            disabled={control.branching}
            onClick={() => control.setBranchDraft(null)}
          >
            Cancel
          </Button>
          <Button
            loading={control.branching}
            disabled={!control.branchDraft?.text.trim()}
            onClick={() => {
              if (control.branchDraft)
                void control.branch(
                  control.branchDraft.message,
                  control.branchDraft,
                );
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
          recovery={selected.limitRecovery}
          openLimits={() => {
            setLimitsOpen(true);
            setLimitsOpenRequest((value) => value + 1);
          }}
          refreshLimits={() => limitRefresh.reload(accountKey, true)}
          newChat={creation.newChat}
          chooseChat={sidebar.showAndFocusSearch}
        />
      )}
      {!layout.usesMenuHeaderTools &&
        headerTools &&
        !room &&
        (opened || managed) &&
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
              {!room && opened && (
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
          {layout.railGeometry && (
            <div className="transcript-heading">
              <SelectedTitle />
            </div>
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
            store={pageStatus}
            id={opened}
            direction="older"
            retry={() => loadPage("older")}
          />
          {loaded &&
            !visibleMessages.length &&
            !notice &&
            !["running", "starting"].includes(agent?.status || "") &&
            (room ? (
              <div className="empty-chat">
                <h2>No messages yet</h2>
                <p>Agent messages will appear here.</p>
              </div>
            ) : (
              <LiveConversationStart agentId={opened || ""} />
            ))}
          {transcript}
          <PageIndicator
            store={pageStatus}
            id={opened}
            direction="newer"
            retry={() => loadPage("newer")}
          />
          {!room && agent && (
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
                control.nextTurnQueue.length > 0
              ) && (
                <LiveAgentPhase
                  agentId={agent.id}
                  phaseOverride={displayPhase}
                  connection={connection}
                  savedMessagePending={delivery.offlinePending}
                  connectionDetails={delivery.offlineDetails}
                />
              )}
          </div>
          {!room && detailedActivity && (
            <LiveAgentPhase
              agentId={agent?.id}
              connection={connection}
              savedMessagePending={delivery.offlinePending}
              connectionDetails={delivery.offlineDetails}
            />
          )}
          <RequestArea
            scope={stateDir}
            refresh={actions.refresh}
            notify={notices.notify}
            onAnswerOpen={onAnswerOpen}
            onAnswerPosition={revealRequest}
          >
            <Requests
              agentId={opened}
              familyId={room?.rootId}
              includeTeam={!mobileClient}
              showDates={!!room}
              excludeIds={inlineRequestIds}
            />
          </RequestArea>
        </div>
      </div>
      {!room && (
        <SelectionQuote
          key={`selection:${opened}`}
          chatId={opened}
          root={scroll}
          onQuote={control.quote}
        />
      )}
      <FollowLatest
        subscribe={subscribeFollow}
        getSnapshot={getFollow}
        onClick={returnToLatest}
      />
      {room ? (
        <p className="room-footer">
          {room.kind === GeneratedApiRoomKind.private
            ? "Private agent chat."
            : "Broadcast to " +
              (room.rootId === "all"
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
                        .catch((error) => notices.notify(errorText(error)))
                    }
                  >
                    Retry queue change
                  </Button>
                </div>
              )}
              <MessageQueue
                loading={messageQueue.loading}
                items={control.nextTurnQueue}
                scope={queueScope}
                onEdit={messageQueue.edit}
                onCancel={messageQueue.cancel}
                onSendNow={controls.steer ? messageQueue.sendNow : undefined}
                onReorder={(ids) =>
                  messageQueue.reorder(
                    mergeQueueOrder(
                      messageQueue.items.map((item) => item.id),
                      control.nextTurnQueue.map((item) => item.id),
                      ids,
                    ),
                  )
                }
                canReorder={
                  messageQueue.canReorder &&
                  !control.nextTurnQueue.some((item) => item.localDelivery)
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
                agent={agent}
                opened={limitsOpen}
                onChange={setLimitsOpen}
                openRequest={limitsOpenRequest}
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
export const LiveConversation = ObservedConversation;
export default ObservedConversation;

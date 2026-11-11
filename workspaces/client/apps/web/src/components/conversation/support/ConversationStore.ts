import { createTranscriptActions } from "../ConversationTranscript/ConversationTranscript.tsx";
import { SkillAutocomplete } from "./SkillAutocomplete.ts";
import { LimitRefresh } from "../../../usage/useLimitRefresh.ts";
import { createUsageView } from "../Usage/Usage.tsx";
import {
  createMessageQueueStore,
  type QueueProps,
} from "../MessageQueue/MessageQueue.tsx";
import { providerSupports } from "../../../generated/providerKind.ts";
import { mergeQueueOrder } from "./messageDelivery.ts";
import { createRealtimeVoice } from "../RealtimeVoice/RealtimeVoice.tsx";
import { createPromptNavigation } from "../PromptNavigator/PromptNavigator.tsx";
import { ClaudeRepairReportStore } from "../ClaudeRepairNotice/ClaudeRepairNotice.tsx";
import {
  resetHistoryWindowAnchor,
  revealHistoryMessage,
} from "./historyWindowModel.ts";
import { isMessagePending } from "../../../messageSendStatus.ts";
import { transcriptMessages } from "../../../conversation/messageView.ts";
import type { KeyboardEvent } from "react";
import type { createConversationContext } from "./conversationContext.ts";

import { useConversationActions } from "./navigation.ts";
import {
  computed,
  compareShallow,
  makeAutoObservable,
  flow,
  reaction,
} from "mobx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { codePointsExceed, sendsTextFile } from "../../../commands.ts";
import { MessageSendStatus, AgentSource } from "../../../generated/api.ts";
import {
  MAX_TURN_ATTACHMENTS,
  MAX_DRAFT_TEXT_CODEPOINTS,
} from "../../../generated/sharedValues.ts";
import { DeliveryMoment } from "../../../generated/deliveryMoment.ts";
import { errorText, type GetResult, type PostResult } from "../../../api.ts";
import { nativeThreadError } from "../../../nativeErrors.ts";
import { copyText } from "../../../clipboard/clipboard.ts";
import type { KnownApiFields, Message } from "../../../types.ts";
import type { Attachment } from "../ComposerAttachments/ComposerAttachments.tsx";
import type {
  LocalQueueItem,
  QueueItem,
} from "../MessageQueue/MessageQueue.tsx";

import { explicitQueue, dispatchedMessage } from "./messageDelivery.ts";
import { isEmptyAssistantMessage } from "./turnHistoryModel.ts";
import { transcriptRequests } from "./questionCardModel.ts";

export type BranchDraft = { message: Message; before: boolean; text: string };
const emptyAttachments: Attachment[] = [];
function mergeAttachments(
  first: readonly Attachment[],
  second: readonly Attachment[],
) {
  return [
    ...new Map(
      [...first, ...second].map((asset) => [asset.id, asset]),
    ).values(),
  ];
}
export const modelCommandPattern = /^\/model(?:\s|$)/i;
export const exactModelCommandPattern = /^\/model$/i;

export class ConversationOperations {
  latestSend: Record<string, symbol> = {};
  branchLock = false;
  branchRequests: Record<string, string> = {};
  uploadLock = false;
}

export type ConversationContext = ReturnType<typeof createConversationContext>;

export class ConversationStore {
  context: ConversationContext;
  readonly autocomplete = new SkillAutocomplete();
  readonly limitRefresh;
  readonly transcriptActions;
  readonly voice;
  readonly promptNavigation;
  readonly repairReport;
  readonly queueEditor;
  readonly usageStore;
  session: ConversationContext["actions"]["selected"]["history"];
  status: {
    direction: "older" | "newer";
    failed: boolean;
  } | null = null;
  paging = false;
  navigationAttempt = 0;
  handledJump = "";
  lastHistoryTop = 0;
  loadPage = flow(function* (
    this: ConversationStore,
    direction: "older" | "newer",
  ) {
    if (this.paging) return;
    const { opened, actions, scrolling } = this.context;
    this.paging = true;
    this.status = { direction, failed: false };
    scrolling.setFollow(false);
    const captureAnchor = () => {
      scrolling.setFollow(false);
      const detail: { id: string | null; endId: string | null } = {
        id: null,
        endId: null,
      };
      scrolling.scroll.current?.dispatchEvent(
        new CustomEvent("studio-history-page-anchor", { detail }),
      );
      this.session.setPageAnchor(detail.id);
      return detail;
    };
    try {
      yield direction === "older"
        ? this.session.older(captureAnchor)
        : this.session.newer(captureAnchor);
      this.status = null;
    } catch (error) {
      if (actions.navigation.selectedId === opened) {
        this.status = { direction, failed: true };
        actions.notices.notify(errorText(error));
      }
    } finally {
      this.paging = false;
    }
  });
  pageAtEdge(direction: "older" | "newer") {
    const { scrolling } = this.context;
    const root = scrolling.scroll.current;
    if (!root || this.paging || this.status?.failed) return;
    const near =
      direction === "older"
        ? this.session.before && root.scrollTop <= root.clientHeight
        : this.session.after &&
          root.scrollHeight - root.scrollTop - root.clientHeight <=
            root.clientHeight;
    if (near && scrolling.consumeHistoryScrollInput())
      void this.loadPage(direction);
  }
  onHistoryScroll() {
    const { scrolling } = this.context;
    const top = scrolling.scroll.current?.scrollTop ?? 0;
    const direction = top < this.lastHistoryTop ? "older" : "newer";
    this.lastHistoryTop = top;
    scrolling.onScroll();
    this.pageAtEdge(direction);
  }
  navigateToMessage = flow(function* (this: ConversationStore, id: string) {
    const { opened, actions, scrolling } = this.context;
    const root = scrolling.scroll.current;
    if (!root) return;
    resetHistoryWindowAnchor(root);
    const attempt = ++this.navigationAttempt;
    scrolling.setFollow(false);
    this.session.setPageAnchor(scrolling.getAnchorId());
    const available: boolean = yield this.session.ensureMessage(id);
    if (
      actions.navigation.selectedId !== opened ||
      this.navigationAttempt !== attempt
    )
      return;
    if (!available)
      throw new Error("This message is unavailable in this conversation.");
    for (let frame = 0; frame < 20; frame++) {
      if (
        scrolling.scroll.current !== root ||
        actions.navigation.selectedId !== opened ||
        this.navigationAttempt !== attempt
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
          scrolling.revealRequest(target, 16);
          this.highlighted = target.dataset.message || null;
          return;
        }
      }
      yield new Promise<void>((resolve) =>
        requestAnimationFrame(() => resolve()),
      );
    }
    throw new Error("The message could not be displayed. Search again.");
  });
  returnToLatest = flow(function* (this: ConversationStore) {
    const { opened, actions, scrolling } = this.context;
    if (scrolling.scroll.current)
      resetHistoryWindowAnchor(scrolling.scroll.current);
    const attempt = ++this.navigationAttempt;
    this.highlighted = null;
    try {
      yield this.session.showLatest();
      if (
        actions.navigation.selectedId === opened &&
        this.navigationAttempt === attempt
      )
        scrolling.setFollow(true);
    } catch (error) {
      actions.notices.notify(errorText(error));
    }
  });

  limitsOpen = false;
  limitsOpenRequest = 0;
  toolsExpanded = false;
  constructor(context: ConversationContext) {
    this.context = context;
    this.limitRefresh = new LimitRefresh(context.api.get);
    this.usageStore = createUsageView(
      context.api,
      context.actions.scope.store,
      () => this.agent?.accountKey || "default",
    );
    this.session = context.actions.selected.history;
    const { actions } = context,
      { services, store } = actions.scope;
    this.voice = createRealtimeVoice({
      agentId: context.opened || "",
      notify: actions.notices.notify,
      store,
      api: services.api,
      audio: services.audio,
      connection: services.net.status,
    });
    this.promptNavigation = createPromptNavigation(
      () => actions.selected.visibleMessages,
    );
    this.repairReport = new ClaudeRepairReportStore(
      services.api,
      () => actions.navigation.selectedAgent,
    );
    this.queueEditor = createMessageQueueStore(
      () => this.queueProps,
      context.actions.scope.services.owner.id,
    );
    makeAutoObservable(
      this,
      {
        context: false,
        transcriptActions: false,
        autocomplete: false,
        limitRefresh: false,
        voice: false,
        promptNavigation: false,
        repairReport: false,
        queueEditor: false,
        usageStore: false,
        lastAssistantByTurn: computed({ equals: compareShallow }),
        inlineRequests: computed({ equals: compareShallow }),
      },
      { autoBind: true, deep: false },
    );
    this.transcriptActions = createTranscriptActions(this);
  }
  start() {
    this.usageStore.start();
    const compact = reaction(
      () => this.context.actions.layout.usesCompactConversationTools,
      (value) => {
        if (!value) this.toolsExpanded = false;
      },
      { fireImmediately: true },
    );
    const jump = reaction(
      () => this.context.actions.navigation.selectedJumpTarget,
      (value) => {
        if (!value || this.handledJump === value.requestId) return;
        this.handledJump = value.requestId;
        this.jumpToPrompt(value.messageId);
      },
      { fireImmediately: true },
    );
    return () => {
      compact();
      jump();
      this.voice.dispose();
      this.repairReport.dispose();
      this.usageStore.dispose();
    };
  }
  get queueProps(): QueueProps {
    const { messageQueue, stateDir, opened, actions } = this.context;
    return {
      items: this.nextTurnQueue,
      scope: `${stateDir}:${actions.startup.workspaceId || ""}:${opened}`,
      onEdit: messageQueue.edit,
      onCancel: messageQueue.cancel,
      onSendNow: providerSupports(this.agent?.provider, "steer")
        ? messageQueue.sendNow
        : undefined,
      onReorder: (ids) =>
        messageQueue.reorder(
          mergeQueueOrder(
            messageQueue.items.map((item) => item.id),
            this.nextTurnQueue.map((item) => item.id),
            ids,
          ),
        ),
      canReorder:
        messageQueue.canReorder &&
        !this.nextTurnQueue.some((item) => item.localDelivery),
      refreshing: messageQueue.busy,
      loading: messageQueue.loading,
      error: messageQueue.error,
    };
  }
  addingFiles = false;
  stopping = false;
  branching = false;
  editLoading = null as string | null;
  highlighted = null as string | null;
  dragging = false;
  removingSending = false;
  editAttempt = 0;
  branchDraft: BranchDraft | null = null;
  get agent() {
    return this.context.actions.navigation.selectedAgent;
  }
  get managed() {
    return this.agent?.source === AgentSource.managed;
  }
  get visibleMessages() {
    return this.context.actions.selected.visibleMessages;
  }
  get assets() {
    const { opened, attachmentDrafts } = this.context;
    return attachmentDrafts.current[opened || ""] || emptyAttachments;
  }
  get pendingFiles() {
    const { opened, uploadRecovery } = this.context;
    return uploadRecovery.pending.filter((row) => row.agent === opened);
  }
  get uploading() {
    return this.addingFiles || this.pendingFiles.length > 0;
  }
  get stopShown() {
    const { opened, actions } = this.context;
    const commands = actions.scope.services.api.pending.list();
    return (
      this.stopping ||
      commands.some(({ kind, target }) => kind === "stop" && target === opened)
    );
  }
  matchesQueue(item: Message, entry: QueueItem) {
    const { opened } = this.context;

    return (
      item.role === "user" &&
      (entry.id === item.clientMessageId ||
        [entry.id, opened + ":" + entry.id].includes(item.id))
    );
  }
  get pendingQueueEntries(): QueueItem[] {
    const { messageQueue } = this.context;

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
        (entry.localDelivery || entry.status === MessageSendStatus.queued) &&
        !visibleMessages.some(
          (item) => dispatchedMessage(item) && this.matchesQueue(item, entry),
        ),
    );
  }
  isNextTurn(entry: QueueItem) {
    return (
      (("delivery" in entry ? entry.delivery : undefined) ||
        entry.requestedDelivery) === DeliveryMoment.NextTurn
    );
  }
  get nextTurnQueue() {
    return this.pendingQueueEntries.filter(this.isNextTurn);
  }
  get currentTurnQueue() {
    return this.pendingQueueEntries.filter((entry) => !this.isNextTurn(entry));
  }
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
  }
  get renderedMessages(): Message[] {
    const { actions } = this.context;

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
  }
  get inlineRequestIds() {
    return new Set(
      [...this.inlineRequests.values()].flatMap((request) =>
        request ? [request.id] : [],
      ),
    );
  }
  get inlineRequests() {
    const { selected, navigation } = this.context.actions;
    const { requestArea } = selected;
    return transcriptRequests(
      this.managed && !navigation.selectedRoom ? this.renderedMessages : [],
      requestArea.requests.requests,
      requestArea.cards,
    );
  }
  *stop() {
    const { opened, api, actions } = this.context;
    if (this.stopShown || !opened) return;
    this.stopping = true;
    try {
      yield api.command.stop(opened, false);
    } catch (error) {
      actions.notices.notify(errorText(error));
    } finally {
      this.stopping = false;
    }
  }
  jumpToPrompt(id: string) {
    void this.navigateToMessage(id).catch((error) =>
      this.context.actions.notices.notify(errorText(error)),
    );
  }
  updateAssets(chat: string, change: (assets: Attachment[]) => Attachment[]) {
    return this.context.attachmentDrafts.set((current) => ({
      ...current,
      [chat]: change(current[chat] || []),
    }));
  }
  *removeAttachment(id: string, agent: string) {
    const { uploadRecovery, actions } = this.context;
    try {
      yield uploadRecovery.remove(id, () => {
        void this.updateAssets(agent, (assets) =>
          assets.filter((asset) => asset.id !== id),
        );
      });
    } catch (error) {
      actions.notices.notify(errorText(error));
    }
  }
  addFiles = flow(function* (
    this: ConversationStore,
    files: globalThis.File[],
  ) {
    const {
      opened,
      stateDir,
      uploadRecovery,
      operations,
      queueUploads,
      actions,
    } = this.context;
    const { startup } = actions;
    const { assets, pendingFiles } = this;
    if (!opened || !this.managed)
      throw new Error("Select a managed chat before attaching files.");
    if (operations.uploadLock)
      throw new Error("Wait for the selected files to be saved, then retry.");
    if (!files.length) throw new Error("Select at least one file.");
    if (
      files.length + assets.length + pendingFiles.length >
      MAX_TURN_ATTACHMENTS
    )
      throw new Error(`Attach at most ${MAX_TURN_ATTACHMENTS} files.`);
    const id = opened;
    operations.uploadLock = true;
    this.addingFiles = true;
    try {
      uploadRecovery.include(
        yield queueUploads(stateDir, id, files, startup.workspaceId || ""),
      );
      uploadRecovery.retry();
    } finally {
      operations.uploadLock = false;
      this.addingFiles = false;
    }
  });
  cannotSend(draft: string) {
    return (
      !this.context.actions.selected.canSend ||
      this.context.actions.delivery.isSending ||
      this.uploading ||
      codePointsExceed(draft, MAX_DRAFT_TEXT_CODEPOINTS) ||
      (!draft.trim() && !this.assets.length)
    );
  }
  recall: {
    version: string | undefined;
    entries: Message[];
    before: string | null;
    loading: boolean;
    request: number;
    index: number;
    value: string;
  } | null = null;
  resetRecall() {
    this.recall = null;
  }
  recallEntries(items: Message[]) {
    return items.filter(
      (item) =>
        item.role === "user" && !isMessagePending(item) && item.text.trim(),
    );
  }
  recallPrompt(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (
      event.defaultPrevented ||
      event.nativeEvent.isComposing ||
      event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey ||
      !["ArrowUp", "ArrowDown"].includes(event.key)
    )
      return false;
    const { actions, opened, kind } = this.context,
      draft = actions.scope.services.drafts.getDraft(opened || "new");
    if (
      this.recall?.version !== this.session.historyVersion ||
      this.recall.value !== draft
    )
      this.resetRecall();
    if (!this.recall) {
      if (draft.length || event.key !== "ArrowUp") return false;
      const entries = this.recallEntries(
          kind === "room" ? [] : this.visibleMessages,
        ),
        before =
          this.managed && kind === "agent" && opened
            ? this.session.before
            : null;
      if (!entries.length && !before) return false;
      this.recall = {
        version: this.session.historyVersion,
        entries,
        index: entries.length,
        value: draft,
        before: before ? String(before) : null,
        loading: false,
        request: 0,
      };
    }
    event.preventDefault();
    const current = this.recall;
    if (event.key === "ArrowDown") {
      current.request++;
      current.loading = false;
    }
    if (current.loading) return true;
    if (event.key === "ArrowUp" && current.index === 0 && current.before) {
      void this.recallEarlier(current);
      return true;
    }
    current.index = Math.max(
      0,
      Math.min(
        current.entries.length,
        current.index + (event.key === "ArrowUp" ? -1 : 1),
      ),
    );
    current.value = current.entries[current.index]?.text || "";
    actions.delivery.editDraft(current.value);
    if (current.index === current.entries.length) this.resetRecall();
    return true;
  }
  *recallEarlier(current: NonNullable<ConversationStore["recall"]>) {
    const { actions, opened, api } = this.context,
      { scope, delivery, notices, selected } = actions;
    const historyVersion = this.session.historyVersion,
      attempt = ++current.request;
    current.loading = true;
    const valid = () =>
      this.recall === current &&
      current.request === attempt &&
      this.session.historyVersion === current.version &&
      actions.navigation.selectedId === opened &&
      scope.services.drafts.getDraft(opened || "new") === current.value;
    try {
      while (current.before && valid()) {
        const before = current.before;
        const slot = scope.store.reads.transcriptPage(api, {
          id: opened || "",
          before,
          limit: 500,
        });
        const page: Awaited<ReturnType<typeof slot.read>> = yield slot.read();
        if (!valid()) return;
        if (page.unavailable) throw new Error(page.unavailable);
        if (
          historyVersion &&
          page.historyVersion &&
          historyVersion !== page.historyVersion
        )
          throw new Error(
            "The conversation changed. Open its message history again.",
          );
        if (page.nextCursor === before)
          throw new Error("Message history did not advance. Try again.");
        const known = new Set(current.entries.map((item) => item.id));
        const earlier = this.recallEntries(
          transcriptMessages(page.items || [], opened || "").filter(
            (item) => !selected.removed.hidden(item),
          ),
        ).filter((item) => !known.has(item.id));
        current.before = page.nextCursor || null;
        if (!earlier.length) continue;
        current.entries = [...earlier, ...current.entries];
        current.index = earlier.length - 1;
        current.value = earlier[current.index]!.text;
        delivery.editDraft(current.value);
        return;
      }
    } catch (error) {
      if (valid()) notices.notify(errorText(error));
    } finally {
      if (current.request === attempt) current.loading = false;
    }
  }
  *submit(
    deliveryMoment: DeliveryMoment = DeliveryMoment.CurrentTurn,
    asTextFile: boolean = false,
  ) {
    const { opened, operations, input, actions } = this.context;
    const { selected, delivery, notices } = actions;
    const { drafts } = actions.scope.services;
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
        this.resetRecall();
        input.current?.focus({ preventScroll: true });
        delivery.editDraft("");
        actions.agentsUi.openSettings(agent.id);
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
    void this.returnToLatest();
    input.current?.focus({ preventScroll: true });
    const sent = new Set(assets.map((asset) => asset.id));
    try {
      yield delivery.send({
        ...(asTextFile ? { asTextFile: true } : {}),
        delivery: deliveryMoment,
        assets: assets.map((asset) => asset.id),
        attachments: assets.map(({ preview: _preview, ...asset }) => asset),
        onPersist: () =>
          this.updateAssets(id, (assets) =>
            assets.filter((asset) => !sent.has(asset.id)),
          ).then(() => {}),
      });
    } catch {
      void this.updateAssets(id, (current) =>
        operations.latestSend[id] !== attempt
          ? current
          : mergeAttachments(assets, current),
      );
      // The send owner restores the draft and renders its delivery error.
    }
  }
  *editMessage(message: Message) {
    const { actions, opened, api } = this.context;
    const { notices } = actions;
    const attempt = ++this.editAttempt;
    const chat = opened;
    this.editLoading = message.id;
    try {
      let original = message;
      if (message.truncated) {
        const full: GetResult<"/api/transcript/item"> = yield api.get(
          "/api/transcript/item",
          {
            query: { id: chat || "", message_id: message.id },
            timeoutMs: 15000,
          },
        );
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
          text: full.text,
        };
      }
      if (
        actions.navigation.selectedId === chat &&
        attempt === this.editAttempt
      )
        this.branchDraft = {
          message: original,
          before: true,
          text: original.text,
        };
    } catch (error) {
      if (
        actions.navigation.selectedId === chat &&
        attempt === this.editAttempt
      )
        notices.notify(errorText(error));
    } finally {
      if (attempt === this.editAttempt) this.editLoading = null;
    }
  }
  *branch(message: Message, draft?: Omit<BranchDraft, "message">) {
    const { actions, opened, operations, api } = this.context;
    const { delivery, notices } = actions;
    const { visibleMessages } = this;
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
    this.branching = true;
    try {
      const response: PostResult<"/api/branch"> = yield api.post(
        "/api/branch",
        {
          agent: agentId,
          message_id: sourceId,
          id: operations.branchRequests[key],
          ...(draft?.before ? { before: true } : {}),
        },
      );
      const branchId = response.id;
      if (draft) {
        const reviewedText =
          draft.before && draft.text === message.text
            ? (response.draft?.text ?? draft.text)
            : draft.text;
        const prefix = draft.before ? response.draft?.prefixText : "";
        delivery.editDraft(
          prefix ? `${prefix}\n\n${reviewedText}` : reviewedText,
          branchId,
        );
        const branchAssets = response.draft?.assets;
        if (draft.before && branchAssets?.length)
          void this.updateAssets(branchId, () => branchAssets);
      }
      actions.open(branchId);
      this.branchDraft = null;
      delete operations.branchRequests[key];
    } catch (error) {
      notices.notify(errorText(error));
    } finally {
      operations.branchLock = false;
      this.branching = false;
    }
  }
  *copy(text: string) {
    const { actions } = this.context;
    const { notices } = actions;
    try {
      yield copyText(text);
      notices.notify("Copied.");
    } catch {
      notices.notify("Clipboard access failed.");
    }
  }
  quote(text: string) {
    const { opened, input, actions } = this.context;
    const { delivery } = actions;
    const { drafts } = actions.scope.services;

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
  }
  *removeSending(messages: Message[]) {
    const { opened, stateDir, kind, removeSendingMessage, actions } =
      this.context;
    const { startup, notices } = actions;
    if (this.removingSending || !opened) return;
    this.removingSending = true;
    try {
      for (const message of messages)
        yield removeSendingMessage(
          { stateDir: stateDir, workspaceId: startup.workspaceId, kind },
          { chat: opened, message },
        );
      notices.notify(
        `Removed ${messages.length} messages. Work already sent continues.`,
      );
    } catch (error) {
      notices.notify(errorText(error));
    } finally {
      this.removingSending = false;
    }
  }
  restoreDraft(m: Message) {
    const { opened, input, actions } = this.context;
    const { delivery } = actions;
    const { drafts } = actions.scope.services;

    const draft = drafts.getDraft(opened || "new");
    const separator =
      draft.trim() && draft.trim() !== m.text.trim() ? "\n\n" : "";
    delivery.editDraft(separator ? draft + separator + m.text : m.text);
    void this.updateAssets(opened || "", (current) =>
      mergeAttachments(current, m.assets || []),
    );
    input.current?.focus({ preventScroll: true });
  }
  *removeMessage(m: Message) {
    const { changeOutbox, actions } = this.context;
    const { delivery, notices } = actions;
    try {
      if (!m.clientMessageId) return;
      yield changeOutbox(m.clientMessageId, "cancel");
      delivery.observeSends?.([m.clientMessageId]);
      notices.notify(
        "Message removed from this device. Delivery was not cancelled.",
      );
    } catch (error) {
      notices.notify(`Could not remove this message: ${errorText(error)}`);
    }
  }
}
export function useConversationStore() {
  const store = useConversationActions().selected.conversation;
  if (!store) throw new Error("Conversation view is not mounted.");
  return store;
}

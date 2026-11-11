import type { UploadRecovery } from "../../components/conversation/support/useUploadRecovery.ts";
import { createConversationContext } from "../../components/conversation/support/conversationContext.ts";
import { AttachmentDrafts } from "../../components/conversation/support/useAttachmentDrafts.ts";
import {
  ConversationStore,
  ConversationOperations,
} from "../../components/conversation/support/ConversationStore.ts";
import { accountLimits } from "../../usage/accountUsage.ts";
import { trimTranscriptPageCache } from "../../transcriptPageBounds.ts";
import { receiptTranscript } from "../../sync/messageReceipts.ts";
import { outgoingTranscript } from "../../components/conversation/support/messageDelivery.ts";
import { isSendingMessage } from "./delivery/removeSendingMessages.ts";
import type { Message } from "../../types.ts";
import { createRemovedMessages } from "./delivery/removedMessages.ts";
import { TranscriptHistory } from "../../conversation/historySession.ts";
import { RoomHistory } from "../../conversation/roomHistory.ts";
import { nativeThreadError } from "../../nativeErrors.ts";
import { RequestAreaOwner } from "../../components/conversation/questions/RequestArea/RequestArea.tsx";
import { createQuestionCards } from "../../components/conversation/support/useQuestionCards.ts";
import type { ServerAreas } from "../ServerAreasProvider.tsx";
import {
  makeAutoObservable,
  computed,
  compareShallow,
  observable,
  observableRef,
  reaction,
} from "mobx";
import {
  AgentSource,
  AgentRole,
  AgentStatus,
  WorkspaceMode,
} from "../../generated/api.ts";
import { SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS } from "../../generated/sharedValues.ts";
import {
  agentId,
  roomId,
  roomLegacy,
  transcriptTail,
} from "../../generated/observableResources.ts";
import { resourceData } from "../../store/runtime.ts";
import {
  currentServerAliases,
  DEFAULT_LOCAL_SERVER_ALIAS,
} from "../../servers/serverAliases.ts";
import { readServerAliases } from "../../servers/registry.ts";
import { errorText } from "../../api.ts";
import { busy } from "../../types.ts";
import { DisplayPhase } from "../../components/conversation/support/useDisplayPhase.ts";
import type { NativeActionOwner } from "./NativeActionOwner.ts";

export class SelectedConversationOwner {
  requestOwner: RequestAreaOwner;
  conversation: ConversationStore | undefined;
  readonly operations = new ConversationOperations();
  attachmentDrafts: AttachmentDrafts;
  stopConversation: (() => void) | undefined;
  closeConversation() {
    if (this.conversation) this.conversation.navigationAttempt++;
    this.conversation?.resetRecall();
    this.requestOwner?.dispose();
    this.stopConversation?.();
    this.stopConversation = undefined;
    this.conversation = undefined;
  }
  get serverAliases() {
    return currentServerAliases(readServerAliases);
  }
  readonly displayPhase;
  get repairReport() {
    return this.conversation?.repairReport;
  }
  suspended = observable.set<string>();
  pendingDeletion = observable.map<string, AbortController>();
  get planType() {
    const key = this.areas.navigation.selectedAgent?.accountKey || "default";
    const state = resourceData(this.areas.scope.store.state().current);
    const account = state?.accountsSummary
      .get("current")
      ?.accounts.find((account) => account.id === key);
    return account
      ? accountLimits(
          state?.rateLimitsByAccount.get(key)?.observation,
          key,
          account.accountId,
        )?.data?.rateLimits?.planType || undefined
      : undefined;
  }
  get limitRecovery() {
    const agent = this.areas.navigation.selectedAgent;
    return agent
      ? resourceData(
          this.areas.scope.store.state().current,
        )?.rateLimitsByAccount.get(agent.accountKey || "default")?.projection
          .agents[agent.id]?.recovery
      : undefined;
  }
  get hasRetainedTeam() {
    const lead = this.areas.navigation.selectedLead;
    return (
      !!lead &&
      !!resourceData(
        this.areas.scope.store.retainedWorktrees(agentId(lead.id)).current,
      )?.length
    );
  }
  stopReaders: (() => void) | undefined;
  history: TranscriptHistory | RoomHistory;
  historyScope = "";
  histories = new Map<string, TranscriptHistory | RoomHistory>();
  historyNamespaces: unknown[][] = [];
  removed: ReturnType<typeof createRemovedMessages>;
  readonly areas;
  constructor(areas: ServerAreas) {
    this.areas = areas;
    this.requestOwner = this.createRequestOwner();
    const {
      startup,
      navigation,
      scope: { services },
    } = areas;
    this.removed = createRemovedMessages(
      startup.creationScope || "",
      this.kind,
      navigation.selectedId,
      services.storage,
    );
    this.attachmentDrafts = new AttachmentDrafts(
      services.uploads.attachmentCancelled,
      areas.notices.notify,
      {
        key: `codex-agent-attachments:${startup.creationScope || ""}`,
        storage: services.storage,
        eventKey: services.storageEventKey,
      },
    );
    this.history = this.createHistory();
    this.historyScope = this.historyKey;
    this.displayPhase = new DisplayPhase(() => [
      areas.navigation.selectedId,
      this.detail?.activity?.phase || "thinking",
    ]);
    makeAutoObservable(
      this,
      {
        areas: false,
        history: observableRef,
        conversation: observableRef,
        requestOwner: observableRef,
        removed: observableRef,
        historyScope: false,
        histories: false,
        historyNamespaces: false,
        displayPhase: false,
        stopReaders: false,
        stopConversation: false,
        team: computed({ equals: compareShallow }),
      },
      { autoBind: true },
    );
  }
  createRequestOwner() {
    const { scope } = this.areas;
    return new RequestAreaOwner(
      {
        api: () => scope.services.api,
        store: scope.store,
        storage: scope.services.storage,
        server: scope.services.owner.id,
      },
      () => ({
        scope: this.areas.startup.creationScope || "",
        notify: this.areas.notices.notify,
        onAnswerOpen: () =>
          this.conversation?.context.scrolling.setFollow(false),
        onAnswerPosition: (node) =>
          this.conversation?.context.scrolling.revealRequest(node),
      }),
      () => ({
        agentId: this.areas.navigation.selectedId,
        familyId: this.areas.navigation.selectedRoom?.rootId,
        includeTeam: !this.areas.layout.isMobile,
      }),
    );
  }
  get receipts() {
    const id = this.areas.navigation.selectedId;
    return new Map(
      [
        ...(resourceData(this.areas.scope.store.state().current)
          ?.messageReceipts ?? []),
      ].filter(([, receipt]) => receipt.agent === id),
    );
  }
  get deliveryTranscript() {
    return outgoingTranscript(
      receiptTranscript(
        this.history.items,
        this.areas.navigation.selectedId || "",
        this.receipts,
      ),
      this.areas.delivery.selectedOutgoing,
    );
  }
  get visibleMessages() {
    const restored = this.removed.restored;
    const matches = (message: Message, copy: Message) =>
      message.id === copy.id ||
      (!!copy.clientMessageId &&
        copy.clientMessageId === message.clientMessageId);
    const items = this.deliveryTranscript.items.map((message) =>
      isSendingMessage(message) && message.deliveryStatus?.source !== "server"
        ? restored.find((copy) => matches(message, copy)) || message
        : message,
    );
    items.push(
      ...restored.filter(
        (copy) =>
          !this.deliveryTranscript.items.some((message) =>
            matches(message, copy),
          ),
      ),
    );
    return receiptTranscript(
      items.filter((message) => !this.removed.hidden(message)),
      this.areas.navigation.selectedId || "",
      this.receipts,
    );
  }
  get historyKey() {
    const { navigation, startup } = this.areas;
    return JSON.stringify([
      navigation.selectedId,
      this.kind,
      this.isManagedConversation,
      startup.creationScope,
      startup.workspaceId,
    ]);
  }
  createHistory() {
    const { store, services } = this.areas.scope;
    const { api, resources } = services;
    const { creationScope: workspace, workspaceId } = this.areas.startup;
    const managed = this.isManagedConversation;
    const dependencies = [
      [store, api, resources, managed, workspace, workspaceId],
      [store, api, workspace, this.kind === "legacy"],
    ];
    dependencies.forEach((next, index) => {
      if (!compareShallow(next, this.historyNamespaces[index])) {
        for (const [key, history] of this.histories)
          if (history instanceof TranscriptHistory === (index === 0))
            this.histories.delete(key);
        this.historyNamespaces[index] = next;
      }
    });
    const key = JSON.stringify([this.kind, this.areas.navigation.selectedId]),
      cached = this.histories.get(key);
    this.histories.delete(key);
    const history = cached ?? this.newHistory();
    this.histories.set(key, history);
    trimTranscriptPageCache(this.histories, key);
    return history;
  }
  newHistory() {
    const {
      scope: {
        services: { api, resources },
        store,
      },
      navigation: { selectedId: id },
    } = this.areas;
    if (this.kind === "agent") {
      const slot = id
        ? store.transcript(
            agentId(id),
            transcriptTail(SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS),
          )
        : undefined;
      return new TranscriptHistory(
        id,
        slot,
        api,
        resources,
        this.isManagedConversation,
      );
    }
    const legacy = this.kind === "legacy";
    const slot = id
      ? store.room(roomId(id), legacy ? roomLegacy(true) : undefined)
      : undefined;
    return new RoomHistory(id, slot, api, legacy);
  }
  get kind() {
    const { navigation } = this.areas;
    return navigation.selectedRoom
      ? "room"
      : navigation.selectedLegacyChat
        ? "legacy"
        : "agent";
  }
  get team() {
    return [
      ...(resourceData(
        this.areas.scope.store.state().current,
      )?.agents.values() || []),
    ];
  }
  get canSend() {
    const { navigation, startup } = this.areas;
    return (
      !startup.schemaMismatch &&
      !nativeThreadError(navigation.selectedAgent, this.detail) &&
      (!!this.detail?.canSend ||
        !!navigation.selectedLegacyChat ||
        !navigation.selectedId)
    );
  }
  get requestArea() {
    const { navigation, scope } = this.areas;
    const requests = this.requestOwner.list;
    return {
      requests,
      cards: createQuestionCards(
        scope.store,
        requests,
        this.isManagedConversation && !navigation.selectedRoom
          ? navigation.selectedAgent?.id
          : undefined,
      ),
    };
  }
  get detail() {
    const id =
      this.isConversationVisible && this.areas.navigation.selectedAgent?.id;
    return id
      ? resourceData(this.areas.scope.store.agentDetail(agentId(id)).current)
      : undefined;
  }
  get readableLead() {
    const { selectedLead: lead } = this.areas.navigation;
    return lead && !this.suspended.has(lead.id) ? lead : undefined;
  }
  get isConversationVisible() {
    return (
      this.areas.scope.screen.active &&
      this.areas.startup.isReady &&
      !(this.areas.layout.isMobile && this.areas.sidebar.mobileOpen) &&
      !this.isSelectionSuspended
    );
  }
  get deletionMessage() {
    const { navigation } = this.areas;
    return navigation.selectedId &&
      this.pendingDeletion.has(navigation.selectedId)
      ? "Deleting chat…"
      : "Project deletion in progress or unconfirmed.";
  }
  get isSelectionSuspended() {
    const { navigation } = this.areas;
    return (
      !!navigation.selectedId &&
      (this.suspended.has(navigation.selectedId) ||
        !!this.pendingDeletion.has(navigation.selectedId))
    );
  }
  get isManagedConversation() {
    return this.areas.navigation.selectedAgent?.source === AgentSource.managed;
  }
  get conversationTitle() {
    const name = this.areas.navigation.selectedLegacyChat?.name;
    return (
      this.areas.navigation.selectedAgent?.name ||
      this.areas.navigation.selectedRoom?.name ||
      (typeof name === "string" ? name : "") ||
      "New conversation"
    );
  }
  get serverAlias() {
    const agent = this.areas.navigation.selectedAgent;
    if (!agent) return undefined;
    const sidebar = this.areas.scope.services.sidebarServices;
    return (
      sidebar.alias?.({ kind: "chat", id: agent.id }) ||
      this.serverAliases[this.areas.scope.services.owner.id] ||
      this.serverAliases[sidebar.serverForChat(agent.id)] ||
      DEFAULT_LOCAL_SERVER_ALIAS
    );
  }
  get serverOffline() {
    const { selectedAgent: agent } = this.areas.navigation;
    return (
      !!agent &&
      !(
        this.areas.scope.services.sidebarServices.available?.({
          kind: "chat",
          id: agent.id,
        }) ?? true
      )
    );
  }
  get canReturnToLead() {
    const { selectedAgent: agent, selectedLead: lead } = this.areas.navigation;
    return (
      this.areas.layout.usesMenuHeaderTools &&
      !!lead &&
      !!agent &&
      !agent.isLead &&
      agent.id !== lead.id
    );
  }
  get canShowTerminalMenu() {
    const { layout } = this.areas;
    return layout.usesMenuHeaderTools && !layout.isMobile;
  }
  get backgroundActivityLabel() {
    return `Current activity${this.backgroundTaskCount ? `, ${this.backgroundTaskCount} active` : ""}`;
  }
  returnToLead() {
    const { navigation } = this.areas;
    if (navigation.selectedLead) this.areas.open(navigation.selectedLead.id);
  }
  toggleTerminal() {
    const screen = this.areas.scope.screen.channel;
    if (screen)
      void screen
        .execute({ action: "terminal", operation: "toggle" })
        .catch((error) => this.areas.notices.notify(String(error)));
    else
      window.dispatchEvent(
        new CustomEvent("studio-terminal-command", { detail: "toggle" }),
      );
  }
  get canStopTeam() {
    const root = this.areas.navigation.selectedLead?.id;
    const agents = resourceData(this.areas.scope.store.state().current)?.agents;
    return (
      this.isManagedConversation &&
      (!!this.areas.navigation.selectedAgent?.inFlight ||
        (!!root &&
          [...(agents?.values() || [])].some(
            (member) =>
              (member.id === root || member.rootId === root) &&
              (busy.has(member.status || "") ||
                member.status === AgentStatus.queued ||
                member.inFlight),
          )))
    );
  }
  get canSwitchToLayr() {
    const { selectedAgent: agent } = this.areas.navigation;
    return (
      this.detail?.workspaceMode === WorkspaceMode.worktree &&
      (agent?.isLead || agent?.role === AgentRole.implementer)
    );
  }
  get isWorkspaceSwitchBlocked() {
    const { selectedAgent: agent } = this.areas.navigation;
    return busy.has(agent?.status || "") || !!agent?.inFlight;
  }
  get backgroundTaskCount() {
    const root = this.isConversationVisible && this.readableLead?.id;
    return root
      ? resourceData(this.areas.scope.store.processes(agentId(root)).current)
          ?.activities.length || 0
      : 0;
  }
  get warningMessages() {
    return this.kind === "agent" ? this.history.items : [];
  }
  get warningScope() {
    const { navigation } = this.areas;
    return `${this.areas.startup.workspaceId || ""}:${navigation.selectedId || ""}:${navigation.selectedAgent?.accountKey || "default"}`;
  }
  nativeAction(...args: Parameters<NativeActionOwner["submit"]>) {
    void this.run(() => this.areas.native.submit(...args));
  }
  *run(command: () => Promise<unknown>) {
    try {
      yield command();
    } catch (error) {
      this.areas.notices.notify(errorText(error));
    }
  }
  switchToLayr() {
    const id = this.areas.navigation.selectedAgent?.id;
    if (id)
      void this.run(() =>
        this.areas.scope.services.api.post(
          "/api/agents/workspace-mode",
          { id, workspaceMode: "layr-native" },
          { timeoutMs: 300000 },
        ),
      );
  }
  stopTeam() {
    const agent = this.areas.navigation.selectedAgent;
    if (agent)
      void this.run(() =>
        this.areas.scope.services.api.command.stop(
          agent.rootId || agent.id,
          true,
        ),
      );
  }
  suspendReaders(scope: ReadonlySet<string>) {
    const selected = this.areas.navigation.selectedId,
      lead = this.readableLead?.id;
    scope.forEach((id) => this.suspended.add(id));
    if ((selected && scope.has(selected)) || (lead && scope.has(lead)))
      this.releaseReaders();
  }
  resumeReaders(scope: ReadonlySet<string>) {
    scope.forEach((id) => this.suspended.delete(id));
  }
  releaseReaders() {
    this.stopReaders?.();
    this.stopReaders = undefined;
  }
  start() {
    const { scope } = this.areas;
    let uploadRecovery: UploadRecovery | undefined;
    let stopUploads: (() => void) | undefined;
    const stopPhase = this.displayPhase.start();
    const stop = reaction<[string, boolean, boolean], true>(
      () => [this.historyKey, this.isConversationVisible, scope.screen.active],
      ([key, active, screenActive]) => {
        this.releaseReaders();
        if (key !== this.historyScope) {
          if (
            this.attachmentDrafts.options?.key !==
            `codex-agent-attachments:${this.areas.startup.creationScope || ""}`
          )
            this.attachmentDrafts = new AttachmentDrafts(
              scope.services.uploads.attachmentCancelled,
              this.areas.notices.notify,
              {
                key: `codex-agent-attachments:${this.areas.startup.creationScope || ""}`,
                storage: scope.services.storage,
                eventKey: scope.services.storageEventKey,
              },
            );
          this.closeConversation();
          this.historyScope = key;
          this.history = this.createHistory();
          this.removed = createRemovedMessages(
            this.areas.startup.creationScope || "",
            this.kind,
            this.areas.navigation.selectedId,
            scope.services.storage,
          );
        }
        if (!screenActive) this.closeConversation();
        if (!this.conversation && screenActive) {
          const context = createConversationContext(
            this.areas,
            this.attachmentDrafts,
            uploadRecovery,
          );
          if (context.uploadRecovery !== uploadRecovery) {
            stopUploads?.();
            uploadRecovery = context.uploadRecovery;
            stopUploads = uploadRecovery.start();
          }
          this.requestOwner = this.createRequestOwner();
          this.requestOwner.start();
          this.conversation = new ConversationStore(context);
          const stopContext = context.start();
          const stopView = this.conversation.start();
          this.stopConversation = () => {
            stopView();
            stopContext();
          };
        }
        this.stopReaders = active ? this.history.start() : undefined;
      },
      { fireImmediately: true, equals: compareShallow },
    );
    const sends = reaction(
      () => this.deliveryTranscript.observed.join(","),
      (ids) => {
        if (ids) this.areas.delivery.observeSends(ids.split(","));
      },
      { fireImmediately: true },
    );
    const boundCache = reaction(
      () => this.history.size,
      () =>
        trimTranscriptPageCache(
          this.histories,
          JSON.stringify([this.kind, this.areas.navigation.selectedId]),
        ),
    );
    return () => {
      sends();
      boundCache();
      this.histories.clear();
      stopPhase();
      stop();
      this.closeConversation();
      stopUploads?.();
      this.releaseReaders();
    };
  }
}

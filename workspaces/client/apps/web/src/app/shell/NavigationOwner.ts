import { makeAutoObservable, reaction } from "mobx";
import { AgentSource } from "../../generated/api.ts";
import { mostRecentActivity } from "../../components/sidebar/catalog.ts";
import type {
  CreatedAgent,
  SelectionIntent,
  ServerAreas,
} from "../contracts.ts";
import { randomUUID } from "../../uuid.ts";

export class NavigationOwner {
  selectedId: string | null = null;
  roomContext: string | null = null;
  jumpTarget:
    | { chatId: string; messageId: string; requestId: string }
    | undefined;
  pendingNavigation:
    | { agentId: string; itemId?: string; requestId: string }
    | undefined;
  navigationIntent = 0;
  selectionScope = "";
  stop: (() => void) | undefined;
  readonly areas: ServerAreas;
  constructor(areas: NavigationOwner["areas"]) {
    this.areas = areas;
    makeAutoObservable(this, { areas: false, stop: false }, { autoBind: true });
  }
  get selectedAgent() {
    return this.selectedId
      ? this.areas.creation.lookup(this.selectedId)
      : undefined;
  }
  get selectedLegacyChat() {
    return this.selectedId
      ? this.areas.state?.chats.get(this.selectedId)
      : undefined;
  }
  get selectedRoom() {
    return this.selectedId
      ? this.areas.state?.rooms.get(this.selectedId)
      : undefined;
  }
  get selectedLead() {
    if (this.selectedRoom?.radio) return undefined;
    const agent = this.selectedAgent;
    const roots = this.selectedRoom?.leadIds ?? [];
    const id =
      agent?.rootId ||
      (agent?.isLead ? agent.id : undefined) ||
      (this.roomContext && roots.includes(this.roomContext)
        ? this.roomContext
        : roots[0]);
    return id ? this.areas.creation.lookup(id) : undefined;
  }
  get availableSelection() {
    return this.selectedAgent ?? this.selectedRoom ?? this.selectedLegacyChat;
  }
  get fallbackSelection() {
    const available = (id: string) => !this.areas.selected.suspended.has(id);
    return (
      mostRecentActivity(
        [...(this.areas.state?.agents.values() ?? [])].filter(
          (agent) =>
            agent.source === AgentSource.managed &&
            agent.isLead &&
            !agent.sharedRoomId &&
            available(agent.id),
        ),
      ) ??
      mostRecentActivity(
        [...(this.areas.state?.rooms.values() ?? [])].filter(
          (room) =>
            room.radio?.direct && !room.userHidden && available(room.id),
        ),
      )
    );
  }
  get selectedJumpTarget() {
    return this.jumpTarget?.chatId === this.selectedId
      ? this.jumpTarget
      : undefined;
  }
  get storageKey() {
    return this.areas.layout.isMobile
      ? "codex-mobile-opened"
      : `codex-desktop-opened:${this.areas.startup.creationScope}`;
  }
  captureIntent() {
    return {
      intent: this.navigationIntent,
      scope: this.areas.startup.creationScope,
    };
  }
  select(model: { readonly id: string }, messageId?: string) {
    this.selectExternal(model.id, {
      ...(messageId ? { messageId } : {}),
      requestId: randomUUID(),
    });
  }
  selectExternal(
    id: string,
    request: { messageId?: string; requestId: string },
  ) {
    if (
      this.areas.selected.suspended.has(id) ||
      this.areas.selected.pendingDeletion.has(id)
    )
      return;
    this.navigationIntent++;
    this.areas.read.select(id);
    this.roomContext = this.selectedLead?.id ?? null;
    this.selectedId = id;
    this.jumpTarget = request.messageId
      ? {
          chatId: id,
          messageId: request.messageId,
          requestId: request.requestId,
        }
      : undefined;
    this.areas.sidebar.close();
  }
  acceptCreated(model: CreatedAgent, captured: SelectionIntent, scope: string) {
    if (
      scope !== this.areas.startup.creationScope ||
      captured.intent !== this.navigationIntent ||
      captured.scope !== this.areas.startup.creationScope
    )
      return false;
    this.select(model);
    return true;
  }
  applyNavigation(request: {
    agentId: string;
    itemId?: string;
    requestId: string;
  }) {
    if (!this.areas.startup.isReady || !this.areas.state) {
      this.pendingNavigation = request;
      return;
    }
    if (!this.areas.state.agents.has(request.agentId))
      this.areas.notices.notify("This chat is no longer available.");
    else {
      this.selectExternal(request.agentId, { requestId: request.requestId });
    }
    this.pendingNavigation = undefined;
  }
  finishDeletion(models: readonly { readonly id: string }[]) {
    if (models.some((model) => model.id === this.selectedId)) {
      this.selectedId = null;
      this.reconcile();
    }
  }
  restoreSelection() {
    if (!this.areas.state) return;
    const scope = `${this.areas.startup.creationScope}:${this.areas.layout.isMobile ? "mobile" : "desktop"}`;
    if (this.selectionScope === scope) return;
    this.selectionScope = scope;
    if (this.areas.layout.isMobile) {
      if (!this.selectedId) this.areas.sidebar.show();
      return;
    }
    const id = this.areas.scope.services.api.saved<string | null>(
      this.storageKey,
      null,
    );
    if (
      id &&
      !this.areas.selected.suspended.has(id) &&
      (this.areas.state.agents.has(id) ||
        this.areas.state.rooms.has(id) ||
        this.areas.state.chats.has(id))
    )
      this.selectedId = id;
  }
  reconcile() {
    if (!this.areas.startup.isReady || !this.areas.state) return;
    this.restoreSelection();
    const roomId = this.selectedAgent?.sharedRoomId;
    const room = roomId ? this.areas.state.rooms.get(roomId) : undefined;
    if (room && !room.userHidden && !this.areas.selected.suspended.has(room.id))
      this.selectedId = room.id;
    if (this.areas.creation.hasUnreplicatedSelection) return;
    if (
      !this.availableSelection &&
      (this.selectedId || !this.areas.layout.isMobile)
    ) {
      this.selectedId = this.areas.layout.isMobile
        ? null
        : (this.fallbackSelection?.id ?? null);
      if (this.areas.layout.isMobile) this.areas.sidebar.show();
    }
    if (this.selectedId)
      this.areas.scope.services.api.save(this.storageKey, this.selectedId);
    if (this.pendingNavigation) this.applyNavigation(this.pendingNavigation);
  }
  start() {
    this.stop?.();
    this.stop = reaction(
      () => [
        this.areas.startup.isReady,
        this.areas.startup.creationScope,
        this.areas.layout.isMobile,
        this.availableSelection,
        this.selectedAgent?.sharedRoomId,
        this.selectedRoom?.userHidden,
        this.fallbackSelection,
        this.areas.creation.hasUnreplicatedSelection,
      ],
      this.reconcile,
      { fireImmediately: true },
    );
    return this.dispose;
  }
  dispose() {
    this.stop?.();
    this.stop = undefined;
  }
}

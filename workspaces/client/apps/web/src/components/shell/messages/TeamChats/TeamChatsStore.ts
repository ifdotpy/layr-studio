import { RoomViewport } from "./RoomViewport.ts";
import { whileObserved } from "../../../../observed.ts";
import { RoomHistory } from "../../../../conversation/roomHistory.ts";
import { roomId } from "../../../../generated/observableResources.ts";
import { trimTranscriptPageCache } from "../../../../transcriptPageBounds.ts";
import { makeAutoObservable, computed, compareShallow, reaction } from "mobx";
import { createArea } from "../../../../app/area.ts";
import { RoomKind as GeneratedApiRoomKind } from "../../../../generated/api.ts";
import type { StateCollections } from "../../../../generated/observableResources.ts";
import type { RequestModel } from "../../../conversation/questions/RequestArea/RequestArea.tsx";
import { createRequestsView } from "../../../conversation/support/requestView.ts";
import { approvalSummary, isPending } from "../../../../approvals.ts";
import { resourceData } from "../../../../store/runtime.ts";
import type {
  useServerApi,
  useServerStore,
} from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
export type MessageRoom =
  StateCollections["rooms"] extends ReadonlyMap<string, infer T> ? T : never;
function personalItems(
  state: StateCollections | undefined,
  requests: readonly RequestModel[],
) {
  return [...(state?.complaints.values() || []), ...requests];
}
function newest<T extends { created?: number | null | undefined }>(
  items: readonly T[],
): T | undefined {
  return items.reduce<T | undefined>(
    (latest, item) =>
      !latest || (item.created || 0) > (latest.created || 0) ? item : latest,
    undefined,
  );
}
function preview(item: ReturnType<typeof personalItems>[number] | undefined) {
  return (
    (item && ("request" in item ? approvalSummary(item) : item.title)) ||
    "Questions, messages and replies"
  );
}

export { preview };
export const [TeamChatsProvider, useTeamChats] =
  createArea<ReturnType<typeof createTeamChatsStore>>("TeamChats");
export function createTeamChatsStore(
  store: ReturnType<typeof useServerStore>,
  api: Pick<ReturnType<typeof useServerApi>, "saved" | "save"> &
    Partial<Pick<ReturnType<typeof useServerApi>, "syncGet">>,
  stateDir: string,
  leadId: string | undefined,
  source?: () => {
    stateDir: string;
    leadId: string | undefined;
    focus?: { id: string; requestId: string; roomId?: string } | undefined;
  },
) {
  const syncGet = api.syncGet;
  const page = (id: string | null) => {
    if (!syncGet) return undefined;
    const history = new RoomHistory(
      id,
      id ? store.room(roomId(id)) : undefined,
      { syncGet },
      false,
    );
    return history;
  };
  const empty = page(null),
    pages = new Map<string, RoomHistory>();
  let stopHistory: (() => void) | undefined;
  const releaseHistory = () => {
    stopHistory?.();
    stopHistory = undefined;
  };
  class TeamChatsStore {
    get stateDir() {
      return source?.().stateDir ?? stateDir;
    }
    get leadId() {
      return source ? source().leadId : leadId;
    }
    get current() {
      return this;
    }
    get messages() {
      return this.room ? pages.get(this.room.id) : empty;
    }
    requests = createRequestsView(store, { all: true });
    readonly viewport = new RoomViewport();
    measureRoom(root: HTMLElement) {
      const messages = this.messages;
      if (messages)
        this.markSeen(
          this.room,
          messages.loaded && !messages.notice,
          this.detail,
          this.viewport.measure(root, messages.items),
        );
    }
    selected = "you";
    detail = false;
    query = "";
    limit = 60;
    seen = new Map<string, number>(
      Object.entries(
        api.saved<Record<string, number>>("codex-chat-seen:" + stateDir, {}),
      ),
    );
    get state() {
      return resourceData(store.state().current);
    }
    get scope() {
      return this.stateDir + ":" + this.leadId;
    }
    get rooms() {
      const member = (id: string) =>
        id === this.leadId ||
        this.state?.agents.get(id)?.rootId === this.leadId;
      return [...(this.state?.rooms.values() || [])].filter(
        (room) =>
          this.leadId &&
          (room.kind === GeneratedApiRoomKind.federated
            ? (room.localMembers ?? room.members ?? []).some((id) => member(id))
            : room.kind === GeneratedApiRoomKind.broadcast
              ? room.rootId === this.leadId
              : !!room.members?.length &&
                (room.members.every((id) => member(id)) ||
                  (!!room.peerTeamId &&
                    room.members.some((id) => member(id))))),
      );
    }
    name(room: MessageRoom) {
      if (room.kind === GeneratedApiRoomKind.federated)
        return room.peerLabel || room.name;
      if (room.kind === GeneratedApiRoomKind.broadcast) return "Team broadcast";
      const peers = (room.members ?? []).filter((id) => id !== this.leadId);
      return peers.length === 1
        ? this.state?.agents.get(peers[0]!)?.name || room.name
        : room.name;
    }
    get room() {
      return this.rooms.find((room) => room.id === this.selected);
    }
    get filtered() {
      return this.rooms
        .filter(
          (item) =>
            !this.query ||
            this.matches(
              this.name(item) + " " + (item.lastMessage?.text || ""),
            ),
        )
        .sort(
          (a, b) =>
            (b.lastMessage?.created ?? b.updated ?? 0) -
            (a.lastMessage?.created ?? a.updated ?? 0),
        );
    }
    get latest() {
      return newest(personalItems(this.state, this.requests.allRequests));
    }
    get attention() {
      return this.requests.allRequests.some(isPending);
    }
    get personal() {
      return !this.query || this.matches("For you " + preview(this.latest));
    }
    get feed() {
      return personalItems(
        this.state,
        this.requests.allRequests.filter(isPending),
      ).sort((a, b) => (a.created || 0) - (b.created || 0));
    }
    matches(text: string) {
      return text.toLowerCase().includes(this.query.toLowerCase());
    }
    choose(id: string) {
      releaseHistory();
      if (!pages.has(id)) {
        const history = page(id);
        if (history) pages.set(id, history);
      }
      trimTranscriptPageCache(pages, id);
      this.selected = id;
      this.detail = true;
      stopHistory = (id === "you" ? empty : pages.get(id))?.start();
    }
    back() {
      releaseHistory();
      this.selected = "you";
      this.detail = false;
    }
    search(query: string) {
      this.query = query;
      this.limit = 60;
    }
    more() {
      this.limit += 60;
    }
    target:
      | {
          roomId?: string | undefined;
          id?: string | undefined;
          requestId?: string | undefined;
        }
      | undefined;
    focus(room?: string, item?: string, request?: string) {
      this.target = { roomId: room, id: item, requestId: request };
      if (room) this.choose(room);
      else if (item || request) this.choose("you");
    }
    markSeen(
      room: MessageRoom | undefined,
      loaded: boolean,
      detailOpen: boolean,
      seq: number,
    ) {
      if (
        !loaded ||
        !detailOpen ||
        !room ||
        (this.seen.get(room.id) || 0) >= seq
      )
        return;
      const key = "codex-chat-seen:" + this.stateDir;
      const next = {
        ...api.saved<Record<string, number>>(key, {}),
        ...Object.fromEntries(this.seen),
        [room.id]: seq,
      };
      api.save(key, next);
      for (const [id, value] of Object.entries(next)) this.seen.set(id, value);
    }
    constructor() {
      makeAutoObservable(
        this,
        {
          rooms: computed({ equals: compareShallow }),
        },
        { autoBind: true },
      );
      whileObserved(this, "current", () => {
        const stop = reaction(
          () => [this.scope, source?.().focus] as const,
          ([scope, focus], previous) => {
            if (scope !== previous?.[0]) {
              if (previous) this.target = undefined;
              this.selected = "you";
              this.query = "";
              this.detail = false;
              this.limit = 60;
              releaseHistory();
              pages.clear();
              this.seen = new Map(
                Object.entries(
                  api.saved<Record<string, number>>(
                    "codex-chat-seen:" + this.stateDir,
                    {},
                  ),
                ),
              );
            }
            const target = focus ?? (!previous ? this.target : undefined);
            if (target) this.focus(target.roomId, target.id, target.requestId);
          },
          { fireImmediately: true, equals: compareShallow },
        );
        return () => {
          stop();
          releaseHistory();
        };
      });
    }
  }
  return new TeamChatsStore();
}

export type TeamChatsOwner = ReturnType<typeof createTeamChatsStore>;

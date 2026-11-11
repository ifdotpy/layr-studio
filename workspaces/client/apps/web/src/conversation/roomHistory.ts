import type { ServerApi } from "../api.ts";
import {
  autorun,
  makeAutoObservable,
  observableRef,
  action,
  flow,
  reaction,
} from "mobx";
import {
  createRoomMirror,
  type RoomData,
  type ChatRead,
  type MessageRecord,
} from "../generated/observableResources.ts";
import { resourceData, type ResourceSlot } from "../store/runtime.ts";
import {
  boundTranscriptItems,
  MAX_TRANSCRIPT_PAGE_BYTES,
} from "../transcriptPageBounds.ts";
import { errorText, type GetResult } from "../api.ts";
import type { Message } from "../types.ts";

type ChatMessage = NonNullable<ChatRead["messages"]>[number];
type CaptureAnchor = () => { id: string | null; endId: string | null };
function legacyRecords(data: RoomData): data is readonly MessageRecord[] {
  return Array.isArray(data);
}

/** Presentation references preserve the generated message identity and fields. */
function createRoomViews() {
  const views = new WeakMap<object, Message>();
  return (data: RoomData): Message[] => {
    const view = (record: ChatMessage | MessageRecord) => {
      let result = views.get(record);
      if (!result) {
        result = {
          get id() {
            return record.id;
          },
          get text() {
            return record.text;
          },
          get kind() {
            return "sender" in record ? "message" : undefined;
          },
          get role() {
            return "author" in record && record.author === "user"
              ? "user"
              : "assistant";
          },
          get at() {
            return "created" in record ? record.created : record.at;
          },
          get created() {
            return "created" in record ? record.created : undefined;
          },
          get seq() {
            return "seq" in record ? record.seq : undefined;
          },
          get sender() {
            return "sender" in record ? record.sender : undefined;
          },
          get senderName() {
            return "senderName" in record
              ? (record.senderName ?? undefined)
              : undefined;
          },
          get sourceId() {
            return "sender" in record ? record.sender : undefined;
          },
          get deliveryStatus() {
            return "status" in record && record.status
              ? { source: "server" as const, status: record.status }
              : undefined;
          },
          get deliveryError() {
            return "error" in record ? (record.error ?? undefined) : undefined;
          },
        };
        views.set(record, result);
      }
      return result;
    };
    return legacyRecords(data)
      ? data.map((record) => view(record))
      : (data.messages || []).map((record) => view(record));
  };
}

/** Room navigation retains generated references, not a second message snapshot. */
export class RoomHistory {
  items: Message[] = [];
  before: string | null = null;
  after: string | null = null;
  pageLoading = false;
  loaded = false;
  notice = "";
  readonly historyVersion = "";
  readonly historical = false;
  readonly live = null;
  readonly connection = null;
  private anchor: string | undefined;
  private active = false;
  private attempt = 0;
  private readonly views = createRoomViews();
  readonly id: string | null;
  private readonly slot: ResourceSlot<RoomData> | undefined;
  private readonly api: Pick<ServerApi, "syncGet">;
  private readonly legacy: boolean;
  constructor(
    id: string | null,
    slot: ResourceSlot<RoomData> | undefined,
    api: Pick<ServerApi, "syncGet">,
    legacy: boolean,
  ) {
    this.id = id;
    this.slot = slot;
    this.api = api;
    this.legacy = legacy;
    makeAutoObservable<this, "slot" | "api" | "views" | "accept">(
      this,
      {
        items: observableRef,
        slot: false,
        api: false,
        views: false,
        accept: action,
        unavailable: action,
      },
      { autoBind: true },
    );
  }
  get size() {
    return this.items.reduce((bytes, item) => bytes + this.sizeOf(item), 0);
  }
  private sizeOf(item: Message) {
    return JSON.stringify(item).length * 2;
  }
  private publish(items: Message[]) {
    if (
      items.length !== this.items.length ||
      items.some((item, index) => item !== this.items[index])
    )
      this.items = items;
  }
  private accept(
    data: RoomData,
    direction?: "older" | "newer",
    captureAnchor?: CaptureAnchor,
  ) {
    const captured = captureAnchor?.();
    if (captured) this.setPageAnchor(captured.id);
    const current = this.slot?.current;
    const canonical = current && resourceData(current);
    const live = new Map(
      (canonical ? this.views(canonical) : []).map((item) => [item.id, item]),
    );
    const source = this.views(data).map((item) => live.get(item.id) || item);
    const overlaps = source.some((item) =>
      this.items.some((prior) => prior.id === item.id),
    );
    const retained = !direction && !overlaps ? [] : this.items;
    const existing =
      direction === "older"
        ? [...source, ...retained]
        : [...retained, ...source];
    const rows = legacyRecords(data)
      ? source
      : [...new Map(existing.map((item) => [item.id, item])).values()].sort(
          (a, b) => (a.seq ?? 0) - (b.seq ?? 0),
        );
    const bounded = boundTranscriptItems(
      rows,
      this.sizeOf,
      direction === "older" ? "oldest" : "newest",
      this.anchor,
      false,
      captured?.endId || undefined,
    );
    this.publish(bounded.items);
    if (!legacyRecords(data)) {
      this.before = bounded.droppedOldest
        ? String(this.items[0]?.seq ?? data.nextBefore ?? "") || null
        : direction === "newer"
          ? this.before
          : String(data.nextBefore ?? "") || null;
      this.after = bounded.droppedNewest
        ? String(this.items.at(-1)?.seq ?? "") || null
        : direction === "older"
          ? this.after
          : String(data.nextAfter ?? "") || null;
    }
    this.loaded = true;
    this.notice = "";
  }
  start() {
    this.active = true;
    const stop = autorun(() => {
      const current = this.slot?.current;
      const data = current && resourceData(current);
      if (data) {
        if (legacyRecords(data)) void data.length;
        else {
          void data.messages;
          void data.nextBefore;
          void data.nextAfter;
        }
        this.accept(data);
      } else if (!this.id || current?.status === "absent") this.unavailable();
      if (current && "error" in current && current.error)
        this.unavailable(current.error);
    });
    const stopBounds = reaction(
      () => this.size,
      (bytes) => {
        if (bytes <= MAX_TRANSCRIPT_PAGE_BYTES) return;
        const bounded = boundTranscriptItems(
          this.items,
          this.sizeOf,
          "newest",
          this.anchor,
        );
        this.publish(bounded.items);
        if (bounded.droppedOldest)
          this.before = String(this.items[0]?.seq ?? "") || null;
        if (bounded.droppedNewest)
          this.after = String(this.items.at(-1)?.seq ?? "") || null;
      },
    );
    return action(() => {
      stopBounds();
      this.active = false;
      this.pageLoading = false;
      this.attempt++;
      stop();
    });
  }
  unavailable(error?: unknown) {
    this.notice = error ? errorText(error) : "";
    this.loaded = true;
    this.items = [];
    this.before = null;
    this.after = null;
  }
  reload = flow(function* (this: RoomHistory) {
    if (!this.id) return;
    const attempt = ++this.attempt;
    this.pageLoading = false;
    try {
      const data: GetResult<"/api/messages"> | GetResult<"/api/agent-chat"> =
        yield this.legacy
          ? this.api.syncGet("/api/messages", { query: { room: this.id } })
          : this.api.syncGet("/api/agent-chat", {
              query: { room: this.id, limit: 100 },
            });
      if (this.active && attempt === this.attempt) {
        const mirror = createRoomMirror(data);
        this.accept(mirror.current);
      }
    } catch (error) {
      if (this.active && attempt === this.attempt) this.unavailable(error);
    }
  });
  private page = flow(function* (
    this: RoomHistory,
    direction: "older" | "newer",
    captureAnchor?: CaptureAnchor,
  ) {
    const cursor = direction === "older" ? this.before : this.after;
    if (!this.id || !cursor || this.pageLoading || this.legacy) return;
    const numeric = Number(cursor);
    if (!Number.isSafeInteger(numeric) || numeric < 0) return;
    const attempt = ++this.attempt;
    this.pageLoading = true;
    try {
      const data: GetResult<"/api/agent-chat"> = yield this.api.syncGet(
        "/api/agent-chat",
        {
          query: {
            room: this.id,
            [direction === "older" ? "before" : "after"]: numeric,
            limit: 100,
          },
        },
      );
      if (!this.active || attempt !== this.attempt) return;
      if (
        direction === "newer" &&
        data.nextAfter != null &&
        data.nextAfter <= numeric
      )
        throw new Error("The room update cursor did not advance.");
      const mirror = createRoomMirror(data);
      this.accept(mirror.current, direction, captureAnchor);
    } finally {
      if (attempt === this.attempt) this.pageLoading = false;
    }
  });
  older(captureAnchor?: CaptureAnchor) {
    return this.page("older", captureAnchor);
  }
  newer(captureAnchor?: CaptureAnchor) {
    return this.page("newer", captureAnchor);
  }
  setPageAnchor(id: string | null) {
    this.anchor = id || undefined;
  }
  showLatest() {
    this.attempt++;
    this.pageLoading = false;
    this.items = [];
    this.before = null;
    this.after = null;
    this.loaded = false;
    return this.reload();
  }
  ensureMessage(id: string) {
    return Promise.resolve(this.items.some((item) => item.id === id));
  }
}

import {
  autorun,
  computed,
  type IComputedValue,
  makeAutoObservable,
  observableRef,
  reaction,
  action,
  flow,
} from "mobx";
import { SubscriptionReplyKind } from "../generated/api.ts";
import { createTranscriptMirror } from "../generated/observableResources.ts";
import { resourceData, type ResourceSlot } from "../store/runtime.ts";
import type {
  useServerApi,
  useServerResources,
} from "../components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS } from "../generated/sharedValues.ts";
import {
  boundTranscriptItems,
  boundTranscriptPage,
  mergeTranscript,
  updateLiveTranscriptPage,
  MAX_TRANSCRIPT_PAGE_BYTES,
} from "../transcriptPageBounds.ts";
import {
  selectMessageConnection,
  type MessageConnection,
} from "../hooks/messageConnection.ts";
import { errorText, type GetResult } from "../api.ts";
import type { Message } from "../types.ts";
import { createMessageViews } from "./messageView.ts";
import { createLiveText } from "./liveText.ts";

type Transcript = ReturnType<typeof createTranscriptMirror>["current"];
type Api = Pick<ReturnType<typeof useServerApi>, "syncGet">;
type Resources = Pick<
  ReturnType<typeof useServerResources>,
  "watchResourceConnection" | "watchResourceChanges" | "watchLiveDeltas"
>;
type CaptureAnchor = () => { id: string | null; endId: string | null };
type PageQuery = {
  before?: string;
  after?: string;
  around?: string;
  captureAnchor?: CaptureAnchor | undefined;
};

/** Navigation and recovery for one selected transcript. The generated facade owns
 * server facts; this owner retains only a bounded window of model references. */
export class TranscriptHistory {
  items: Message[] = [];
  before: string | null | undefined = null;
  after: string | null = null;
  historical = false;
  pageLoading = false;
  notice = "";
  loaded = false;
  connection: MessageConnection | null = null;
  readonly live = createLiveText();
  private version = "";
  private latest: Transcript | undefined;
  private anchor: string | undefined;
  private attempt = 0;
  private active = false;
  private readonly views;
  private readonly byteSizes = new WeakMap<Message, IComputedValue<number>>();
  readonly id: string | null;
  private readonly slot: ResourceSlot<Transcript> | undefined;
  private readonly api: Api;
  private readonly resources: Resources;
  private readonly managed: boolean;
  constructor(
    id: string | null,
    slot: ResourceSlot<Transcript> | undefined,
    api: Api,
    resources: Resources,
    managed = true,
  ) {
    this.id = id;
    this.slot = slot;
    this.api = api;
    this.resources = resources;
    this.managed = managed;
    this.views = createMessageViews(id);
    makeAutoObservable<
      this,
      "latest" | "slot" | "api" | "resources" | "views" | "byteSizes" | "accept"
    >(
      this,
      {
        items: observableRef,
        connection: observableRef,
        latest: observableRef,
        slot: false,
        api: false,
        resources: false,
        views: false,
        byteSizes: false,
        live: false,
        accept: action,
        unavailable: action,
        failed: action,
      },
      { autoBind: true },
    );
  }
  get size() {
    return this.items.reduce((bytes, item) => bytes + this.sizeOf(item), 0);
  }
  get historyVersion() {
    return this.version;
  }
  // Observe all serialized fields so mutable text and nested metadata invalidate.
  private sizeOf(item: Message) {
    let size = this.byteSizes.get(item);
    if (!size) {
      size = computed(() => JSON.stringify(item).length * 2);
      this.byteSizes.set(item, size);
    }
    return size.get();
  }
  private publish(items: Message[]) {
    if (
      items.length !== this.items.length ||
      items.some((item, index) => item !== this.items[index])
    )
      this.items = items;
  }
  private resetWindow() {
    this.historical = false;
    this.before = null;
    this.after = null;
    this.items = [];
    this.attempt++;
    this.pageLoading = false;
    this.live.clear();
  }
  private accept(data: Transcript) {
    if (
      data.unavailable ||
      (this.version &&
        data.historyVersion &&
        this.version !== data.historyVersion)
    ) {
      this.resetWindow();
    }
    this.latest = data;
    this.version = data.historyVersion || "";
    const incoming = this.views(data.items);
    const page = {
      items: this.items,
      before: this.before,
      focused: this.historical,
    };
    if (!this.managed || !this.items.length) {
      page.items = incoming;
      page.before =
        data.nextCursor || (data.truncated ? data.items[0]?.id : null);
    } else updateLiveTranscriptPage(page, incoming, data);
    const bounded = boundTranscriptItems(
      page.items,
      this.sizeOf,
      this.historical ? "oldest" : "newest",
      this.historical ? this.anchor : undefined,
    );
    this.publish(bounded.items);
    this.before = bounded.droppedOldest
      ? this.items[0]?.id || page.before
      : page.before;
    if (bounded.droppedNewest && this.historical)
      this.after = this.items.at(-1)?.id || this.after;
    if (!this.historical) this.after = null;
    this.notice =
      data.unavailable ||
      (data.truncated && !this.managed ? "Recent messages only." : "");
    this.loaded = true;
  }
  start() {
    this.active = true;
    const stop = autorun(() => {
      const current = this.slot?.current;
      const data = current && resourceData(current);
      // Only membership/metadata tracks here. Text fields belong to their leaves.
      if (data) {
        data.items.forEach((record) => {
          record.inputs?.forEach((input) => input.id);
        });
        void data.historyVersion;
        void data.nextCursor;
        void data.truncated;
        void data.unavailable;
        this.accept(data);
      } else if (!this.id || current?.status === "absent") this.unavailable();
      if (current && "error" in current && current.error)
        this.failed(current.error);
    });
    const stopConnection =
      this.id && this.managed
        ? this.resources.watchResourceConnection(
            action((health, activity, detail) => {
              if (health !== "live" || activity !== "open") this.live.clear();
              const next = selectMessageConnection(health, activity, detail);
              if (
                this.connection?.health !== next?.health ||
                this.connection?.activity !== next?.activity ||
                this.connection?.message !== next?.message
              )
                this.connection = next;
            }),
          )
        : () => {};
    const stopReceipt =
      this.id && this.managed
        ? this.resources.watchResourceChanges(
            {
              kind: "transcript",
              agentId: this.id,
              tail: SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS,
            },
            action((version) => {
              if (version?.replyKind !== SubscriptionReplyKind.snapshot) return;
              this.resetWindow();
              const current = this.slot?.current;
              const data = current && resourceData(current);
              if (data) this.accept(data);
            }),
            { waitForBaseline: true },
          )
        : () => {};
    const stopLive = this.id
      ? this.resources.watchLiveDeltas((event) => {
          if (event.agentId === this.id) this.live.add(event);
        })
      : () => {};
    const stopDurable = reaction(
      () => this.items.map((item) => [item.id, item.text] as const),
      (next, previous) => {
        const old = new Map(previous);
        for (const [id, text] of next)
          if (old.has(id) && old.get(id) !== text) this.live.remove(id);
      },
    );
    const stopBounds = reaction(
      () => this.size,
      (bytes) => {
        if (bytes <= MAX_TRANSCRIPT_PAGE_BYTES) return;
        const bounded = boundTranscriptItems(
          this.items,
          this.sizeOf,
          this.historical ? "oldest" : "newest",
          this.anchor,
        );
        this.publish(bounded.items);
        if (bounded.droppedOldest)
          this.before = this.items[0]?.id || this.before;
        if (bounded.droppedNewest)
          this.after = this.items.at(-1)?.id || this.after;
      },
    );
    return action(() => {
      this.active = false;
      this.attempt++;
      stop();
      stopConnection();
      stopReceipt();
      stopLive();
      stopDurable();
      stopBounds();
      this.pageLoading = false;
      this.live.clear();
    });
  }
  unavailable() {
    this.resetWindow();
    this.loaded = true;
    this.latest = undefined;
    this.notice = this.id ? "This session is no longer available." : "";
  }
  failed(error: unknown) {
    this.notice = errorText(error);
    this.loaded = true;
  }
  reload = flow(function* (this: TranscriptHistory) {
    if (!this.id) return;
    const attempt = ++this.attempt;
    this.pageLoading = false;
    try {
      const dto: GetResult<"/api/transcript"> = yield this.api.syncGet(
        "/api/transcript",
        {
          query: { id: this.id },
        },
      );
      if (!this.active || attempt !== this.attempt) return;
      const mirror = createTranscriptMirror(dto);
      this.accept(mirror.current);
    } catch (error) {
      if (this.active && attempt === this.attempt) this.failed(error);
    }
  });
  private fetchPage = flow(function* (
    this: TranscriptHistory,
    query: PageQuery,
  ) {
    if (!this.id || !this.managed || (this.pageLoading && !query.around))
      return false;
    const attempt = ++this.attempt;
    this.pageLoading = true;
    try {
      const result: GetResult<"/api/transcript/page"> = yield this.api.syncGet(
        "/api/transcript/page",
        {
          query: {
            id: this.id,
            limit: SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS,
            ...(query.before ? { before: query.before } : {}),
            ...(query.after ? { after: query.after } : {}),
            ...(query.around ? { around: query.around } : {}),
          },
        },
      );
      if (!this.active || attempt !== this.attempt) return false;
      if (
        result.historyVersion &&
        this.latest?.historyVersion &&
        result.historyVersion !== this.latest.historyVersion
      )
        throw new Error(
          "The conversation changed. Search or load its history again.",
        );
      const page = createTranscriptMirror(result);
      // Current tail models remain canonical for overlap. A focused page must
      // keep observing those same fields while live data changes in place.
      const live = new Map(
        this.views(this.latest?.items || []).map((item) => [item.id, item]),
      );
      const source = this.views(page.current.items).map(
        (item) => live.get(item.id) || item,
      );
      const captured = query.captureAnchor?.();
      const anchor = captured?.id || this.anchor || query.around;
      const merged =
        query.around || (!query.before && !query.after)
          ? source
          : query.after
            ? mergeTranscript(this.items, source)
            : mergeTranscript(source, this.items);
      const bounded = boundTranscriptPage(
        merged,
        this.sizeOf,
        query.before
          ? "older"
          : query.after
            ? "newer"
            : query.around
              ? "around"
              : "latest",
        {
          before: this.before || null,
          after: this.after,
          nextBefore: result.nextCursor || null,
          nextAfter: result.nextAfterCursor || null,
        },
        anchor,
        captured?.endId || undefined,
      );
      this.publish(bounded.items);
      this.before = bounded.before;
      this.after = bounded.after;
      this.historical =
        !!query.around || (!!(query.before || query.after) && this.historical);
      this.anchor = anchor;
      this.loaded = true;
      return true;
    } finally {
      if (attempt === this.attempt) this.pageLoading = false;
    }
  });
  older(captureAnchor?: CaptureAnchor): Promise<void> {
    return this.before
      ? this.fetchPage({ before: this.before, captureAnchor }).then(() => {})
      : Promise.resolve();
  }
  newer(captureAnchor?: CaptureAnchor): Promise<void> {
    return this.after
      ? this.fetchPage({ after: this.after, captureAnchor }).then(() => {})
      : Promise.resolve();
  }
  setPageAnchor(id: string | null) {
    this.anchor = id || undefined;
  }
  ensureMessage(id: string) {
    return this.items.some((item) => item.id === id || item.sourceId === id)
      ? Promise.resolve(true)
      : this.fetchPage({ around: id });
  }
  showLatest(): Promise<void> {
    this.attempt++;
    this.pageLoading = false;
    if (this.after && !this.historical)
      return this.fetchPage({}).then(() => {});
    if (this.historical) {
      this.historical = false;
      this.after = null;
      this.items = [];
      if (this.latest) this.accept(this.latest);
    }
    return Promise.resolve();
  }
}

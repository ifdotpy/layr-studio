import { SubscriptionReplyKind } from "../generated/api.ts";
import { subscriptionKey } from "./subscriptions/contracts.ts";
import { SYNC_PROTOCOL_VERSION, type components } from "../generated/api.ts";
import { MAX_SYNC_RESOURCE_COUNT } from "../generated/sharedValues.ts";

type Batch = components["schemas"]["EntityChangeBatch"];
type Frame = components["schemas"]["ResourceEntityTransferEvent"];
export type StagedEntityInterval = Omit<
  Frame,
  "data" | "partIndex" | "partCount" | "complete"
> & {
  after: number;
  resourceEntry?: components["schemas"]["ResourceRevisionEntry"];
  pages(): AsyncIterable<Batch>;
  has(id: string): Promise<boolean>;
  dispose(): Promise<void>;
};
export interface TransferStorage {
  put(key: string, value: string): Promise<void>;
  putMany(values: [string, string][]): Promise<void>;
  get(key: string): Promise<string | undefined>;
  remove(prefix: string): Promise<void>;
  close(): Promise<void>;
}

/** Staging in page memory, for a page whose projections live in the native core.
 * An interval is applied only after it is complete, so a lost page loses nothing durable. */
export class MemoryTransferStorage implements TransferStorage {
  private readonly pages = new Map<string, string>();
  put(key: string, value: string) {
    this.pages.set(key, value);
    return Promise.resolve();
  }
  putMany(values: [string, string][]) {
    for (const [key, value] of values) this.pages.set(key, value);
    return Promise.resolve();
  }
  get(key: string) {
    return Promise.resolve(this.pages.get(key));
  }
  remove(prefix: string) {
    for (const key of this.pages.keys())
      if (key.startsWith(prefix)) this.pages.delete(key);
    return Promise.resolve();
  }
  close() {
    this.pages.clear();
    return Promise.resolve();
  }
}

function sequence(value: number): boolean {
  return Number.isSafeInteger(value) && value >= 0;
}
function frameValue(value: Frame): Frame {
  if (
    value.protocol !== SYNC_PROTOCOL_VERSION ||
    !sequence(value.revision) ||
    value.cursor.workspaceId !== value.workspaceId ||
    !sequence(value.cursor.seq) ||
    (value.cursor.terminalGeneration !== undefined &&
      !sequence(value.cursor.terminalGeneration)) ||
    !sequence(value.pageIndex) ||
    !sequence(value.partIndex) ||
    !sequence(value.partCount) ||
    value.partCount < 1 ||
    value.partIndex >= value.partCount ||
    (value.complete && value.partIndex + 1 !== value.partCount)
  )
    throw new TypeError("Invalid bounded entity transfer frame");
  return value;
}
/** The schema hash fences DTO shape; sequence checks fence each staged interval. */
function pageValue(value: Batch, after: number, through: number): Batch {
  if (
    !sequence(value.after) ||
    value.after !== after ||
    !sequence(value.through) ||
    value.through < after ||
    value.through > through
  )
    throw new TypeError("Invalid canonical entity page interval");
  let previous = after;
  for (const row of value.documents) {
    if (
      !sequence(row.seq) ||
      row.seq < previous ||
      row.seq <= after ||
      row.seq > value.through
    )
      throw new TypeError("Invalid canonical entity page record");
    previous = row.seq;
  }
  return value;
}
type Pending = {
  frame: Frame;
  page: number;
  part: number;
  after: number | undefined;
  through: number | undefined;
  complete: boolean;
};

/** Only interval metadata stays in memory. Serialized fragments and validated
 * pages remain in temporary storage until the complete interval is finalized. */
export class EntityTransferAssembler {
  private readonly pending = new Map<string, Pending>();
  private generation = 0;
  declare private readonly storage: TransferStorage;
  constructor(storage: TransferStorage = new MemoryTransferStorage()) {
    this.storage = storage;
  }
  async invalidate() {
    this.generation++;
    this.pending.clear();
    await this.storage.close();
  }
  /** Called after queued fragment work settles; unrelated staging stays intact. */
  async remove(resources: components["schemas"]["ResourceRef"][]) {
    const removed = new Set(resources.map(subscriptionKey));
    for (const [scope, pending] of this.pending) {
      if (!removed.has(subscriptionKey(pending.frame.resource))) continue;
      this.pending.delete(scope);
      // Completed intervals belong to their FIFO operation until disposal.
      if (!pending.complete)
        await this.storage.remove(`${pending.frame.transferId}:`);
    }
  }
  async receive(value: Frame): Promise<StagedEntityInterval | undefined> {
    const frame = frameValue(value);
    const generation = this.generation;
    const scope = JSON.stringify(frame.resource);
    let pending = this.pending.get(scope);
    const prefix = `${frame.transferId}:`;
    const key = `${prefix}fragment:${frame.pageIndex}:${frame.partIndex}`;
    if (
      pending &&
      (pending.frame.workspaceId !== frame.workspaceId ||
        pending.frame.epoch !== frame.epoch ||
        pending.frame.transferId !== frame.transferId)
    ) {
      // Completed intervals belong to their FIFO operation until disposal.
      if (!pending.complete)
        await this.storage.remove(`${pending.frame.transferId}:`);
      this.pending.delete(scope);
      pending = undefined;
    }
    if (!pending) {
      if (
        frame.pageIndex !== 0 ||
        frame.partIndex !== 0 ||
        this.pending.size >= MAX_SYNC_RESOURCE_COUNT
      )
        throw new Error("Entity transfer has no initial page");
      pending = {
        frame,
        page: 0,
        part: 0,
        after: undefined,
        through: undefined,
        complete: false,
      };
      this.pending.set(scope, pending);
    }
    const first = pending.frame;
    if (
      frame.cursor.seq !== first.cursor.seq ||
      frame.cursor.terminalGeneration !== first.cursor.terminalGeneration ||
      frame.replyKind !== first.replyKind ||
      frame.revision !== first.revision ||
      frame.reason !== first.reason ||
      frame.payloadKind !== first.payloadKind
    )
      throw new Error("Entity transfer identity changed");
    if (
      frame.pageIndex < pending.page ||
      (frame.pageIndex === pending.page && frame.partIndex < pending.part) ||
      pending.complete
    ) {
      if ((await this.storage.get(key)) !== frame.data)
        throw new Error("Conflicting entity transfer replay");
      return;
    }
    if (
      frame.pageIndex !== pending.page ||
      frame.partIndex !== pending.part ||
      (frame.partIndex > 0 && frame.partCount !== pending.frame.partCount)
    )
      throw new Error("Entity transfer part out of order");
    if (frame.partIndex === 0) pending.frame = frame;
    await this.storage.put(key, frame.data);
    if (generation !== this.generation) return;
    pending.part++;
    if (pending.part !== frame.partCount) return;
    const chunks: string[] = [];
    for (let index = 0; index < frame.partCount; index++) {
      const chunk = await this.storage.get(
        `${prefix}fragment:${frame.pageIndex}:${index}`,
      );
      if (chunk === undefined)
        throw new Error("Missing entity transfer fragment");
      chunks.push(chunk);
    }
    const encoded = chunks.join("");
    const decoded: Batch = JSON.parse(encoded);
    if (frame.payloadKind !== "entity") {
      if (frame.pageIndex !== 0 || !frame.complete)
        throw new Error("Incomplete resource transfer page");
      // The schema hash fences typed payloads; only the interval identity is checked here.
      const entry: components["schemas"]["ResourceRevisionEntry"] =
        JSON.parse(encoded);
      if (
        !sequence(entry.revision) ||
        entry.cursor?.workspaceId !== frame.workspaceId ||
        entry.cursor.seq !== frame.cursor.seq ||
        entry.cursor.terminalGeneration !== frame.cursor.terminalGeneration ||
        entry.replyKind !== frame.replyKind ||
        !entry.resource ||
        subscriptionKey(entry.resource) !== subscriptionKey(frame.resource)
      )
        throw new Error("Resource transfer identity changed");
      pending.complete = true;
      return {
        ...frame,
        after: 0,
        resourceEntry: entry,
        async *pages() {},
        has: async () => false,
        dispose: async () => {
          await this.storage.remove(prefix);
          if (this.pending.get(scope) === pending) this.pending.delete(scope);
        },
      };
    }
    const after = pending.through ?? decoded.after;
    const batch = pageValue(decoded, after, frame.cursor.seq);
    if (pending.after === undefined) {
      pending.after = batch.after;
      if (
        frame.replyKind === SubscriptionReplyKind.snapshot &&
        batch.after !== 0
      )
        throw new Error("Snapshot transfer has a nonzero base");
    }
    await this.storage.putMany([
      [`${prefix}page:${frame.pageIndex}`, encoded],
      ...batch.documents.map((row): [string, string] => [
        `${prefix}id:${row.collection}:${row.id}`,
        "1",
      ]),
    ]);
    pending.through = batch.through;
    pending.page++;
    pending.part = 0;
    if (!frame.complete) return;
    if (pending.through !== frame.cursor.seq)
      throw new Error("Incomplete entity transfer interval");
    pending.complete = true;
    const storage = this.storage;
    const pageCount = pending.page;
    const intervalAfter = pending.after;
    const isCurrent = () => generation === this.generation;
    return {
      ...frame,
      after: intervalAfter,
      async *pages() {
        let after = intervalAfter;
        for (let index = 0; index < pageCount; index++) {
          if (!isCurrent()) throw new Error("Entity transfer cancelled");
          const encoded = await storage.get(`${prefix}page:${index}`);
          if (encoded === undefined)
            throw new Error("Missing staged entity page");
          const batch = pageValue(JSON.parse(encoded), after, frame.cursor.seq);
          after = batch.through;
          yield batch;
        }
      },
      has: async (id) => (await storage.get(`${prefix}id:${id}`)) !== undefined,
      dispose: async () => {
        await storage.remove(prefix);
        if (this.pending.get(scope) === pending) this.pending.delete(scope);
      },
    };
  }
}

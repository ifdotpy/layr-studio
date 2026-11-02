import {
  SubscriptionCachePolicy,
  type SyncEntityPayload,
  type components,
} from "../generated/api";
import { API_SCHEMA_HASH } from "../generated/apiSchema";
import type { LocalCollection, Row } from "./localCollection";
import { subscriptionKey, type ResourceRef } from "./subscriptions/contracts";
import { SUBSCRIPTION_ENTITY_SCOPES } from "../generated/subscriptionEntities";
import { cursorDocumentId } from "./subscriptions/cursors";
import { parseEntityPayload } from "./entityProjection";
import { readTranscriptMeta } from "./transcriptMetadata";

/** Minimum delay between scheduled cache writes, in milliseconds. */
export const SUBSCRIPTION_CACHE_WRITE_MS = 5_000;
/** Current on-device subscription record format version. */
export const SUBSCRIPTION_CACHE_VERSION = 2;
/** Largest total size of all subscription records on this device, in bytes (UTF-8 of the stored JSON). */
export const SUBSCRIPTION_CACHE_MAX_BYTES = 32 * 1024 * 1024;
/** Key of the index record: the bytes and last-use counter of each subscription record. */
export const SUBSCRIPTION_CACHE_INDEX_KEY = "subscription-cache:index";
type Cursor = Pick<
  components["schemas"]["SubscriptionCursor"],
  "workspaceId" | "seq"
>;
type CachedRow =
  | { kind: "entity"; id: string; seq: number; entity: SyncEntityPayload }
  | { kind: "transcript"; id: string; seq: number; payload: string };
type RecordValue = {
  version: typeof SUBSCRIPTION_CACHE_VERSION;
  contractHash: typeof API_SCHEMA_HASH;
  subscription: string;
  cursor: Cursor;
  data: CachedRow[];
};
export type SubscriptionCacheStorage = {
  get(key: string): Promise<unknown>;
  put(key: string, value: unknown): Promise<void>;
  delete(key: string): Promise<void>;
};

type IndexEntry = { bytes: number; use: number };
type Index = Record<string, IndexEntry>;

function byteLength(value: unknown) {
  return new TextEncoder().encode(JSON.stringify(value)).byteLength;
}

function readIndex(raw: unknown): Index {
  if (!raw || typeof raw !== "object") return {};
  const valid = (Object.values(raw) as unknown[]).every(
    (entry) =>
      !!entry &&
      typeof entry === "object" &&
      "bytes" in entry &&
      typeof entry.bytes === "number" &&
      "use" in entry &&
      typeof entry.use === "number",
  );
  return valid ? (raw as Index) : {};
}

function totalBytes(index: Index) {
  return Object.values(index).reduce((sum, entry) => sum + entry.bytes, 0);
}

function nextUse(index: Index) {
  return 1 + Math.max(0, ...Object.values(index).map((entry) => entry.use));
}

/** Least recently used key of the index, never the key `except`. */
function leastRecentlyUsed(index: Index, except: string) {
  let victim: [string, IndexEntry] | undefined;
  for (const [key, entry] of Object.entries(index))
    if (key !== except && (!victim || entry.use < victim[1].use))
      victim = [key, entry];
  return victim?.[0];
}

function transcriptRow(ref: ResourceRef, id: string) {
  return (
    ref.kind === "transcript" &&
    (id === `transcript:${ref.agentId}` ||
      id.startsWith(`transcript:item:${ref.agentId}:`))
  );
}

function hasOnlyKeys(value: object, keys: readonly string[]) {
  return Object.keys(value).every((key) => keys.includes(key));
}

// Full generated DTO decoding is tracked by work-1793131928531406336-8.

function decode(
  value: unknown,
  ref: ResourceRef,
  workspace: string,
): { cursor: Cursor; rows: Row[] } | undefined {
  const scopes: Record<
    string,
    { collections: readonly string[]; idField: string | null }
  > = SUBSCRIPTION_ENTITY_SCOPES;
  const scope = scopes[ref.kind];
  const selector: Readonly<Record<string, unknown>> = ref;
  if (
    !value ||
    typeof value !== "object" ||
    !hasOnlyKeys(value, [
      "version",
      "contractHash",
      "subscription",
      "cursor",
      "data",
    ]) ||
    !("version" in value) ||
    value.version !== SUBSCRIPTION_CACHE_VERSION ||
    !("contractHash" in value) ||
    value.contractHash !== API_SCHEMA_HASH ||
    !("subscription" in value) ||
    value.subscription !== subscriptionKey(ref) ||
    !("cursor" in value) ||
    !value.cursor ||
    typeof value.cursor !== "object" ||
    !hasOnlyKeys(value.cursor, ["workspaceId", "seq"]) ||
    !("workspaceId" in value.cursor) ||
    value.cursor.workspaceId !== workspace ||
    !("seq" in value.cursor) ||
    typeof value.cursor.seq !== "number" ||
    !Number.isSafeInteger(value.cursor.seq) ||
    value.cursor.seq < 0 ||
    !("data" in value) ||
    !Array.isArray(value.data)
  )
    return undefined;
  const cursor: Cursor = { workspaceId: workspace, seq: value.cursor.seq };
  const rows: Row[] = [];
  const ids = new Set<string>();
  for (const raw of value.data) {
    const row: unknown = raw;
    if (
      !row ||
      typeof row !== "object" ||
      !("kind" in row) ||
      !("id" in row) ||
      typeof row.id !== "string" ||
      !("seq" in row) ||
      typeof row.seq !== "number" ||
      !Number.isSafeInteger(row.seq) ||
      row.seq < 0 ||
      row.seq > cursor.seq ||
      ids.has(row.id)
    )
      return undefined;
    ids.add(row.id);
    if (
      row.kind === "entity" &&
      hasOnlyKeys(row, ["kind", "id", "seq", "entity"])
    ) {
      const entity = "entity" in row ? parseEntityPayload(row.entity) : null;
      if (
        !entity ||
        (entity.collection === "agentDetail" &&
          entity.value.id !== entity.id) ||
        ("id" in entity.value && entity.value.id !== entity.id) ||
        !scope ||
        !scope.collections.includes(entity.collection) ||
        row.id !== `entity:${entity.collection}:${entity.id}` ||
        (scope.idField && selector[scope.idField] !== entity.id)
      )
        return undefined;
      rows.push({
        id: row.id,
        seq: row.seq,
        payload: "",
        _deleted: false,
        entity,
      });
    } else if (
      row.kind === "transcript" &&
      hasOnlyKeys(row, ["kind", "id", "seq", "payload"]) &&
      transcriptRow(ref, row.id) &&
      "payload" in row &&
      typeof row.payload === "string"
    ) {
      rows.push({
        id: row.id,
        seq: row.seq,
        payload: row.payload,
        _deleted: false,
      });
    } else return undefined;
  }
  if (ref.kind === "transcript") {
    const base = rows.find((row) => row.id === `transcript:${ref.agentId}`);
    const metadata = base && readTranscriptMeta(base.payload);
    if (
      !metadata ||
      !metadata.order.every(
        (item) => typeof item === "string" && item.length > 0,
      ) ||
      new Set(metadata.order).size !== metadata.order.length
    )
      return undefined;
    for (const item of metadata.order) {
      const row = rows.find(
        (entry) => entry.id === `transcript:item:${ref.agentId}:${item}`,
      );
      if (!row) return undefined;
      try {
        const value: unknown = JSON.parse(row.payload);
        if (
          !value ||
          typeof value !== "object" ||
          Array.isArray(value) ||
          !("id" in value) ||
          value.id !== item
        )
          return undefined;
      } catch {
        return undefined;
      }
    }
  }
  return { cursor, rows };
}

function encode(
  ref: ResourceRef,
  cursor: Cursor,
  rows: Row[],
): CachedRow[] | undefined {
  const result: CachedRow[] = [];
  for (const row of rows) {
    if (row._deleted || row.evicted) continue;
    if (row.id.startsWith("entity:")) {
      if (!row.entity || row.seq > cursor.seq) return undefined;
      result.push({
        kind: "entity",
        id: row.id,
        seq: row.seq,
        entity: row.entity,
      });
    } else if (transcriptRow(ref, row.id)) {
      if (row.seq > cursor.seq) return undefined;
      result.push({
        kind: "transcript",
        id: row.id,
        seq: row.seq,
        payload: row.payload,
      });
    }
  }
  return result;
}

export function createSubscriptionCache(
  storage: SubscriptionCacheStorage,
  collection: LocalCollection,
  workspace: string,
  { maxBytes = SUBSCRIPTION_CACHE_MAX_BYTES }: { maxBytes?: number } = {},
) {
  const key = (ref: ResourceRef) =>
    JSON.stringify([workspace, subscriptionKey(ref)]);
  const pending = new Map<string, RecordValue>();
  const policies = new Map<
    string,
    components["schemas"]["SubscriptionCachePolicy"]
  >();
  const loaded = new Map<string, Promise<Cursor | undefined>>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  // Index changes run one at a time, so a read and a write never interleave.
  let queue: Promise<unknown> = Promise.resolve();
  function indexed<T>(task: () => Promise<T>): Promise<T> {
    const run = queue.then(task);
    queue = run.catch(() => undefined);
    return run;
  }
  async function writeRecord(index: Index, id: string, value: RecordValue) {
    const bytes = byteLength(value);
    if (bytes > maxBytes) {
      await storage.delete(id);
      delete index[id];
      return;
    }
    let total = totalBytes(index) - (index[id]?.bytes ?? 0) + bytes;
    while (total > maxBytes) {
      const victim = leastRecentlyUsed(index, id);
      if (victim === undefined) break;
      await storage.delete(victim);
      total -= index[victim]?.bytes ?? 0;
      delete index[victim];
    }
    await storage.put(id, value);
    index[id] = { bytes, use: nextUse(index) };
  }
  const writeRecords = (values: [string, RecordValue][]) =>
    indexed(async () => {
      const index = readIndex(await storage.get(SUBSCRIPTION_CACHE_INDEX_KEY));
      for (const [id, value] of values)
        await writeRecord(index, id, value).catch(() => {});
      await storage.put(SUBSCRIPTION_CACHE_INDEX_KEY, index);
    });
  const touch = (id: string, bytes: number) =>
    indexed(async () => {
      const index = readIndex(await storage.get(SUBSCRIPTION_CACHE_INDEX_KEY));
      index[id] = { bytes, use: nextUse(index) };
      await storage.put(SUBSCRIPTION_CACHE_INDEX_KEY, index);
    });
  async function flush() {
    clearTimeout(timer);
    timer = undefined;
    const values = [...pending];
    pending.clear();
    await writeRecords(values).catch(() => {});
  }
  const hide = () => {
    if (document.visibilityState === "hidden") void flush();
  };
  if (typeof document !== "undefined")
    document.addEventListener("visibilitychange", hide);
  return {
    load(ref: ResourceRef): Promise<Cursor | undefined> {
      const id = key(ref);
      const previous = loaded.get(id);
      if (previous) return previous;
      const result: Promise<Cursor | undefined> = (async () => {
        try {
          const raw = await storage.get(id);
          if (raw === undefined) return undefined;
          const value = decode(raw, ref, workspace);
          if (!value) {
            await storage.delete(id);
            return undefined;
          }
          const cursorId = await cursorDocumentId(ref);
          const markers: Row[] =
            ref.kind === "state"
              ? [
                  {
                    id: "state:entities:checkpoint",
                    seq: value.cursor.seq,
                    payload: "{}",
                  },
                  {
                    id: "state:entities:ready",
                    seq: value.cursor.seq,
                    payload: "ready",
                  },
                  {
                    id: "state:entities:complete",
                    seq: value.cursor.seq,
                    payload: "complete",
                  },
                ]
              : [
                  {
                    id: cursorId,
                    seq: value.cursor.seq,
                    payload: JSON.stringify(value.cursor),
                  },
                ];
          const applied = await collection.hydrate(
            [...value.rows, ...markers],
            () =>
              loaded.get(id) === result &&
              policies.get(id) !== SubscriptionCachePolicy.noCache,
          );
          if (applied) await touch(id, byteLength(raw)).catch(() => {});
          return applied ? value.cursor : undefined;
        } catch {
          return undefined;
        }
      })();
      loaded.set(id, result);
      return result;
    },
    async setPolicy(
      ref: ResourceRef,
      policy: components["schemas"]["SubscriptionCachePolicy"],
    ) {
      const id = key(ref);
      policies.set(id, policy);
      if (policy === SubscriptionCachePolicy.noCache) {
        pending.delete(id);
        loaded.delete(id);
        await storage.delete(id);
      }
    },
    canSave(ref: ResourceRef) {
      return policies.get(key(ref)) === SubscriptionCachePolicy.cache;
    },
    clearPolicies() {
      policies.clear();
      pending.clear();
      loaded.clear();
    },
    save(ref: ResourceRef, cursor: Cursor, data: Row[]) {
      if (policies.get(key(ref)) !== SubscriptionCachePolicy.cache) return;
      const encoded = encode(ref, cursor, data);
      if (!encoded) return;
      pending.set(key(ref), {
        version: SUBSCRIPTION_CACHE_VERSION,
        contractHash: API_SCHEMA_HASH,
        subscription: subscriptionKey(ref),
        cursor: { workspaceId: cursor.workspaceId, seq: cursor.seq },
        data: encoded,
      });
      timer ??= setTimeout(() => void flush(), SUBSCRIPTION_CACHE_WRITE_MS);
    },
    stop() {
      clearTimeout(timer);
      if (typeof document !== "undefined")
        document.removeEventListener("visibilitychange", hide);
    },
  };
}

/** Copy only rows that form this subscription's resumable projection. */
export async function subscriptionRows(
  ref: ResourceRef,
  collection: LocalCollection,
): Promise<Row[]> {
  const cursorId = await cursorDocumentId(ref);
  const rows = collection.all();
  if (ref.kind === "transcript") {
    const itemPrefix = `transcript:item:${ref.agentId}:`;
    return rows.filter(
      (row) =>
        row.id === cursorId ||
        row.id === `transcript:${ref.agentId}` ||
        row.id.startsWith(itemPrefix),
    );
  }
  const scopes: Record<
    string,
    {
      collections: readonly string[];
      idField: string | null;
    }
  > = SUBSCRIPTION_ENTITY_SCOPES;
  const scope = scopes[ref.kind];
  if (!scope) return [];
  const selector: Readonly<Record<string, unknown>> = ref;
  return rows.filter((row) => {
    if (
      row.id === cursorId ||
      (ref.kind === "state" && row.id.startsWith("state:entities:"))
    )
      return true;
    const collectionName = row.id.split(":")[1];
    if (
      !row.id.startsWith("entity:") ||
      !scope.collections.includes(collectionName ?? "")
    )
      return false;
    if (
      scope.idField &&
      row.id !== `entity:${collectionName}:${selector[scope.idField]}`
    )
      return false;
    return true;
  });
}

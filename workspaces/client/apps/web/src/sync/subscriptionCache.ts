import type { components } from "../generated/api";
import { API_SCHEMA_HASH } from "../generated/apiSchema";
import type { LocalCollection, Row } from "./localCollection";
import { subscriptionKey, type ResourceRef } from "./subscriptions/contracts";
import { SUBSCRIPTION_ENTITY_SCOPES } from "../generated/subscriptionEntities";
import { cursorDocumentId, parseAppliedCursor } from "./subscriptions/cursors";

/** Minimum delay between scheduled cache writes, in milliseconds. */
export const SUBSCRIPTION_CACHE_WRITE_MS = 5_000;
const VERSION = 1;
/** Only these subscriptions have a cursor and a resumable local projection. */
export function cacheableSubscription(ref: ResourceRef) {
  return (
    ref.kind in SUBSCRIPTION_ENTITY_SCOPES ||
    ref.kind === "transcript" ||
    ref.kind === "terminal-output"
  );
}
type Cursor = components["schemas"]["SubscriptionCursor"];
type RecordValue = {
  version: typeof VERSION;
  kind: ResourceRef["kind"];
  cursor: Cursor;
  data: Row[];
};
export type SubscriptionCacheStorage = {
  get(key: string): Promise<unknown>;
  put(key: string, value: RecordValue): Promise<void>;
  delete(key: string): Promise<void>;
};

function decode(
  value: unknown,
  ref: ResourceRef,
  workspace: string,
): RecordValue | undefined {
  if (
    !value ||
    typeof value !== "object" ||
    !("version" in value) ||
    value.version !== VERSION ||
    !("kind" in value) ||
    value.kind !== ref.kind ||
    !("cursor" in value) ||
    !value.cursor ||
    typeof value.cursor !== "object" ||
    !("workspaceId" in value.cursor) ||
    value.cursor.workspaceId !== workspace ||
    !("seq" in value.cursor) ||
    typeof value.cursor.seq !== "number" ||
    !Number.isSafeInteger(value.cursor.seq) ||
    value.cursor.seq < 0 ||
    !("data" in value) ||
    !Array.isArray(value.data) ||
    !value.data.every(
      (row: unknown) =>
        row !== null &&
        typeof row === "object" &&
        "id" in row &&
        typeof row.id === "string" &&
        "seq" in row &&
        typeof row.seq === "number" &&
        Number.isSafeInteger(row.seq) &&
        row.seq >= 0 &&
        "payload" in row &&
        typeof row.payload === "string",
    )
  )
    return undefined;
  // Payloads are the generated projection contracts; the envelope is checked once here.
  return value as RecordValue;
}

export function createSubscriptionCache(
  storage: SubscriptionCacheStorage,
  collection: LocalCollection,
  workspace: string,
) {
  const key = (ref: ResourceRef) =>
    JSON.stringify([workspace, API_SCHEMA_HASH, subscriptionKey(ref)]);
  const pending = new Map<string, RecordValue>();
  const loaded = new Map<string, Promise<Cursor | undefined>>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  async function flush() {
    clearTimeout(timer);
    timer = undefined;
    const values = [...pending];
    pending.clear();
    await Promise.all(
      values.map(([id, value]) => storage.put(id, value).catch(() => {})),
    );
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
      const result = (async () => {
        try {
          const raw = await storage.get(id);
          if (raw === undefined) return undefined;
          const value = decode(raw, ref, workspace);
          const awaitCursorId = await cursorDocumentId(ref);
          const cursorRow = value?.data.find(
            (row) =>
              row.id ===
              (ref.kind === "state"
                ? "state:entities:checkpoint"
                : awaitCursorId),
          );
          const storedCursor = parseAppliedCursor(
            cursorRow?.payload,
            workspace,
          );
          if (
            !value ||
            !cursorRow ||
            cursorRow._deleted ||
            (ref.kind === "state"
              ? cursorRow.seq !== value.cursor.seq ||
                !value.data.some(
                  (row) =>
                    row.id === "state:entities:ready" &&
                    row.payload === "ready" &&
                    !row._deleted,
                ) ||
                !value.data.some(
                  (row) =>
                    row.id === "state:entities:complete" &&
                    row.payload === "complete" &&
                    !row._deleted,
                )
              : storedCursor?.seq !== value.cursor.seq ||
                storedCursor.terminalGeneration !==
                  value.cursor.terminalGeneration)
          ) {
            await storage.delete(id);
            return undefined;
          }
          await collection.hydrate(value.data);
          return value.cursor;
        } catch {
          return undefined;
        }
      })();
      loaded.set(id, result);
      return result;
    },
    save(ref: ResourceRef, cursor: Cursor, data: Row[]) {
      pending.set(key(ref), { version: VERSION, kind: ref.kind, cursor, data });
      timer ??= setTimeout(() => void flush(), SUBSCRIPTION_CACHE_WRITE_MS);
    },
    flush,
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
  if (ref.kind === "terminal-output")
    return rows.filter((row) => row.id === cursorId);
  const scopes: Record<
    string,
    {
      collections: readonly string[];
      idField: string | null;
      agentField: string | null;
      includeTeamMembers: boolean;
    }
  > = SUBSCRIPTION_ENTITY_SCOPES;
  const scope = scopes[ref.kind];
  if (!scope) return [];
  const selector: Readonly<Record<string, unknown>> = ref;
  const agent = scope.agentField ? selector[scope.agentField] : undefined;
  const agents = new Set<unknown>([agent]);
  if (agent && scope.includeTeamMembers)
    for (const row of rows) {
      if (
        row.entity?.collection === "agentSummary" &&
        row.entity.value.rootId === agent
      )
        agents.add(row.entity.id);
    }
  return rows.filter((row) => {
    if (
      row.id === cursorId ||
      (ref.kind === "state" && row.id.startsWith("state:entities:"))
    )
      return true;
    if (
      agent &&
      scope.includeTeamMembers &&
      row.entity?.collection === "agentSummary" &&
      row.entity.value.rootId === agent
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
    if (!agent || !row.entity) return true;
    const value = row.entity.value;
    return (
      ("agent" in value && agents.has(value.agent)) ||
      ("rootId" in value && value.rootId === agent)
    );
  });
}

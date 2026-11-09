import { autorun } from "mobx";
import {
  TRANSIENT_RESOURCE_KINDS,
  createProjectionMetadataMirror,
} from "../generated/observableResources.ts";
import { SubscriptionReplyKind } from "../generated/api.ts";
import { mergeTerminalOutput } from "./subscriptions/terminalOutput.ts";
import { SUBSCRIPTION_ENTITY_SCOPES } from "../generated/subscriptionEntities.ts";
import {
  cursorDocumentId,
  parseAppliedCursor,
} from "./subscriptions/cursors.ts";
import { serverDatabaseSuffix } from "../servers/environment.ts";

import { SYNC_ENTITY_STREAM_MAX_DOCUMENTS } from "../generated/sharedValues.ts";

import { LocalCollection } from "./localCollection.ts";
import {
  createSubscriptionCache,
  subscriptionRows,
  type SubscriptionCacheStorage,
} from "./subscriptionCache.ts";

import type { EntityRow } from "./entityContracts.ts";

import {
  syncGet,
  syncPost,
  saved,
  save,
  setWorkspace,
  isApiSchemaMismatch,
} from "../api.ts";

import type { SyncEntityPayload } from "../generated/api.ts";

import {
  readTranscriptMeta,
  TRANSCRIPT_PROJECTION_FORMAT,
  type TranscriptProjectionMeta,
} from "./transcriptMetadata.ts";

import {
  getEntitySequenceCheckpoint,
  setEntitySequenceProjection,
} from "./entitySequence.ts";

import {
  watchResourceChanges,
  watchResourceConnection,
  replaceStream,
  type ResourceRef,
  type ResourceVersion,
} from "./subscriptions/events.ts";

import { onResume } from "./resume.ts";

import {
  cacheTranscriptValue,
  peekTranscript,
  patchTranscriptValue,
  subscribeTranscript,
} from "./transcriptCache.ts";

import { API_SCHEMA_HASH } from "../generated/apiSchema.ts";

import type { components } from "../generated/api.ts";

import { withEntityProjectionWrite } from "./localCollection.ts";

import { entityProjectionDatabaseName } from "./entityCacheStorage.ts";

export type SyncDocument = EntityRow;

/** Derive the local primary key once; entities remain structured in memory. */
export function entityCacheRow(
  change: components["schemas"]["SyncEntity"],
): EntityRow {
  const { op, seq, ...entity } = change;
  return {
    id: `entity:${change.collection}:${change.id}`,
    seq,
    payload: "",
    _deleted: op === "delete",
    ...(op === "upsert" ? { entity: entity as SyncEntityPayload } : {}),
  };
}

export {
  watchResourceChanges,
  watchResourceConnection,
} from "./subscriptions/events.ts";

export type { ResourceRef } from "./subscriptions/events.ts";

type TranscriptSyncPayload = components["schemas"]["TranscriptSyncPayload"];

import type { TranscriptProjectionPayload } from "../apiTypes.ts";
export type { TranscriptProjectionPayload } from "../apiTypes.ts";

import { clearTranscript } from "./transcriptCache.ts";
export type SyncClientOptions = {
  cacheStorage?: SubscriptionCacheStorage | undefined;
  onFreshReceipts?: (
    workspaceId: string,
    receipts: import("./messageReceipts.ts").MessageReceipt[],
  ) => void | Promise<void>;
  api: Pick<
    import("../api.ts").ServerApi,
    | "syncGet"
    | "syncPost"
    | "saved"
    | "save"
    | "setWorkspace"
    | "isApiSchemaMismatch"
  >;
  resources: Pick<
    typeof import("./subscriptions/events.ts"),
    "watchResourceChanges" | "watchResourceConnection"
  > & { replaceStream?: () => void };
  sequences: Pick<
    ReturnType<typeof import("./entitySequence.ts").createEntitySequenceScope>,
    "getEntitySequenceCheckpoint" | "setEntitySequenceProjection"
  >;
  transcripts: ReturnType<
    typeof import("./transcriptCache.ts").createTranscriptCache
  >;
  databaseSuffix: string;
};
export function createSyncClient(options: SyncClientOptions) {
  const metadata = createProjectionMetadataMirror({
    workspaceId: "",
    ready: false,
    complete: false,
    projectSequences: {},
  });
  const { syncGet, saved, save, setWorkspace } = options.api;
  const { watchResourceChanges, watchResourceConnection } = options.resources;
  const { setEntitySequenceProjection } = options.sequences;
  const {
    clearTranscript,
    cacheTranscriptValue,
    peekTranscript,
    patchTranscriptValue,
    subscribeTranscript,
  } = options.transcripts;
  const serverDatabaseSuffix = options.databaseSuffix;

  let pending: ReturnType<typeof open> | undefined;
  let subscriptionCache: ReturnType<typeof createSubscriptionCache> | undefined;
  let cachePolicySession = {};
  async function freshReceipts(
    workspaceId: string,
    documents: components["schemas"]["SyncEntity"][],
  ) {
    const receipts = documents.flatMap((row) =>
      row.op === "upsert" && row.collection === "messageReceipt"
        ? [row.value]
        : [],
    );
    if (receipts.length) await options.onFreshReceipts?.(workspaceId, receipts);
  }

  async function open() {
    const cached = saved<string>("codex-sync-workspace", "");
    const hasCache = /^[a-f0-9]{32}$/.test(cached);
    const identify = async () => {
      if (navigator.onLine === false)
        throw new TypeError("The device is offline.");
      const identity = await syncGet("/api/sync/identity");
      if (!/^[a-f0-9]{32}$/.test(identity.workspaceId))
        throw new Error("Invalid workspace identity");
      return identity;
    };
    const initialIdentity = identify();
    let identity: Awaited<ReturnType<typeof identify>> | null = null;
    // Cached display opens at once; every applied stream interval fences workspace identity.
    if (hasCache) initialIdentity.catch(() => {});
    else identity = await initialIdentity;
    const workspaceId = identity?.workspaceId || cached;
    setEntitySequenceProjection(workspaceId, API_SCHEMA_HASH);
    if (identity) save("codex-sync-workspace", workspaceId);
    setWorkspace(workspaceId);
    metadata.apply({ ...metadata.current, workspaceId });
    const db = {
      projections: new LocalCollection(
        entityProjectionDatabaseName(
          workspaceId,
          API_SCHEMA_HASH,
          serverDatabaseSuffix,
        ),
      ),
    };
    if (options.cacheStorage)
      subscriptionCache = createSubscriptionCache(
        options.cacheStorage,
        db.projections,
        workspaceId,
      );
    db.projections.$.subscribe((event) => {
      const doc = event.documentData;
      if (doc.id === "state:entities:ready") {
        const ready = !doc._deleted && doc.payload === "ready";
        metadata.apply({
          ...metadata.current,
          ready,
          complete: ready && metadata.current.complete,
        });
      } else if (doc.id === "state:entities:complete")
        metadata.apply({
          ...metadata.current,
          complete: !doc._deleted && doc.payload === "complete",
        });
      else if (doc.id.startsWith("entity:project:")) {
        const projectSequences = { ...metadata.current.projectSequences };
        const id = doc.id.slice("entity:project:".length);
        if (doc._deleted || doc.evicted) delete projectSequences[id];
        else projectSequences[id] = doc.seq;
        metadata.apply({ ...metadata.current, projectSequences });
      }
      if (doc.id.startsWith("transcript:") && doc.id.split(":").length === 2)
        cacheStoredTranscriptAfterWrite(
          db.projections,
          workspaceId,
          doc.id,
          doc,
        );
    });
    return { db, workspaceId };
  }

  function syncDatabase() {
    return (pending ??= open().catch((error) => {
      pending = undefined;
      throw error;
    }));
  }

  async function readSubscriptionCursor(resource: ResourceRef) {
    if (TRANSIENT_RESOURCE_KINDS.has(resource.kind)) return undefined;
    await syncDatabase();
    await subscriptionCache?.load(resource);
    return readAppliedSubscriptionCursor(resource);
  }

  async function readAppliedSubscriptionCursor(resource: ResourceRef) {
    const { db, workspaceId } = await syncDatabase();
    if (resource.kind === "state") {
      // A completed state interval owns this checkpoint and its baseline markers.
      const rows = await db.projections.getRows(
        [
          "state:entities:checkpoint",
          "state:entities:ready",
          "state:entities:complete",
        ],
        true,
      );
      const checkpoint = rows.find(
        (row) => row.id === "state:entities:checkpoint",
      );
      const ready = rows.find((row) => row.id === "state:entities:ready");
      const complete = rows.find((row) => row.id === "state:entities:complete");
      if (
        checkpoint &&
        !checkpoint._deleted &&
        Number.isSafeInteger(checkpoint.seq) &&
        checkpoint.seq >= 0 &&
        ready?.payload === "ready" &&
        !ready._deleted &&
        complete?.payload === "complete" &&
        !complete._deleted
      )
        return { workspaceId, seq: checkpoint.seq };
      return undefined;
    }
    const [row] = await db.projections.getRows(
      [await cursorDocumentId(resource)],
      false,
    );
    const cursor = parseAppliedCursor(row?.payload, workspaceId);
    if (cursor && resource.kind === "transcript") {
      const [base] = await db.projections.getRows(
        [`transcript:${resource.agentId}`],
        false,
      );
      if (!base || base.seq !== cursor.seq || !readTranscriptMeta(base.payload))
        return undefined;
    }
    return cursor;
  }

  async function cacheSubscription(resource: ResourceRef) {
    const cache = subscriptionCache;
    if (!cache || !cache.canSave(resource)) return;
    const { db } = await syncDatabase();
    await withEntityProjectionWrite(db.projections.database.name, async () => {
      const cursor = await readAppliedSubscriptionCursor(resource);
      if (!cursor) return;
      cache.save(
        resource,
        cursor,
        await subscriptionRows(resource, db.projections),
      );
    });
  }

  async function persistSubscriptionCursor(
    resource: ResourceRef,
    cursor: components["schemas"]["SubscriptionCursor"],
    isCurrent: () => boolean,
  ) {
    const { db, workspaceId } = await syncDatabase();
    if (!isCurrent() || workspaceId !== cursor.workspaceId) return;
    if (resource.kind === "state") return;
    await persistProjection(db.projections, {
      id: await cursorDocumentId(resource),
      payload: JSON.stringify(cursor),
      seq: cursor.seq,
    });
  }

  async function persistScopedEntities(
    resource: ResourceRef,
    entry: components["schemas"]["ResourceRevisionEntry"],
    isCurrent: () => boolean,
  ) {
    const cursor = entry.cursor;
    const changes = entry.entityChanges;
    if (!cursor || !changes) return false;
    const scopes: Record<
      string,
      {
        collections: readonly string[];
        idField: string | null;
        agentField: string | null;
        includeTeamMembers: boolean;
      }
    > = SUBSCRIPTION_ENTITY_SCOPES;
    const scope = scopes[resource.kind];
    if (!scope) return false;
    const { db, workspaceId } = await syncDatabase();
    if (
      cursor.workspaceId !== workspaceId ||
      cursor.seq !== entry.revision ||
      changes.through !== cursor.seq
    )
      throw new Error("Invalid subscription cursor interval");
    const selector = resource as Record<string, unknown>;
    const selectedAgent = scope.agentField
      ? selector[scope.agentField]
      : undefined;
    const selectedAgents = new Set([selectedAgent]);
    if (selectedAgent && scope.includeTeamMembers) {
      const summaries = await db.projections
        .find({
          selector: {
            id: { $gte: "entity:agentSummary:", $lt: "entity:agentSummary;" },
          },
        })
        .exec();
      for (const row of summaries) {
        const entity = row.toJSON().entity;
        if (
          entity?.collection === "agentSummary" &&
          entity.value.rootId === selectedAgent
        )
          selectedAgents.add(entity.id);
      }
    }
    const belongs = (entity: {
      collection: string;
      id: string;
      value?: unknown;
    }) => {
      if (!scope.collections.includes(entity.collection)) return false;
      if (scope.idField && entity.id !== selector[scope.idField]) return false;
      if (
        scope.agentField &&
        selector[scope.agentField] &&
        entity.value &&
        !(
          scope.includeTeamMembers &&
          selectedAgents.has((entity.value as { agent?: string }).agent)
        ) &&
        (entity.value as { rootId?: string }).rootId !==
          selector[scope.agentField]
      )
        return false;
      return true;
    };
    await withEntityProjectionWrite(db.projections.database.name, async () => {
      if (!isCurrent()) return;
      const applied = await readAppliedSubscriptionCursor(resource);
      if (applied && applied.seq >= cursor.seq) return;
      // Another tab or a mutation response may already cover an interval prefix.
      // Entity writes are idempotent; only a missing base or forward gap fails.
      if (
        entry.replyKind !== SubscriptionReplyKind.snapshot &&
        (!applied || applied.seq < changes.after)
      )
        throw new Error("Subscription delta has no applied local base");
      if (
        changes.documents.some(
          (row) =>
            !belongs(row) ||
            row.seq > cursor.seq ||
            (entry.replyKind !== SubscriptionReplyKind.snapshot &&
              row.seq <= changes.after),
        )
      )
        throw new Error("Entity outside subscription interval");
      const rows = changes.documents.map(entityCacheRow);
      if (entry.replyKind === SubscriptionReplyKind.snapshot) {
        const incoming = new Set(rows.map((row) => row.id));
        for (const collection of scope.collections) {
          const prefix = `entity:${collection}:`;
          const existing = await db.projections
            .find({
              selector: { id: { $gte: prefix, $lt: `entity:${collection};` } },
            })
            .exec();
          for (const doc of existing) {
            const row = doc.toJSON();
            // A different scope may have committed a source version beyond this snapshot.
            if (
              row.seq <= cursor.seq &&
              row.entity &&
              belongs(row.entity) &&
              !incoming.has(row.id)
            )
              rows.push({
                id: row.id,
                payload: "",
                seq: row.seq,
                _deleted: true,
                evicted: true,
              });
          }
        }
      }
      await persistProjectionBatch(db.projections, rows, isCurrent);
      if (!isCurrent()) return;
      if (resource.kind === "state") {
        await persistProjection(db.projections, {
          id: "state:entities:checkpoint",
          payload: "{}",
          seq: cursor.seq,
        });
        if (entry.replyKind === SubscriptionReplyKind.snapshot) {
          const [ready] = await db.projections.getRows(
            ["state:entities:ready"],
            true,
          );
          await persistProjection(db.projections, {
            id: "state:entities:complete",
            payload: "complete",
            seq: Date.now(),
          });
          await persistProjection(db.projections, {
            id: "state:entities:ready",
            payload: "ready",
            seq: (ready?.seq ?? 0) + 1,
          });
        }
      }
      await persistSubscriptionCursor(resource, cursor, isCurrent);
    });
    return true;
  }

  /** Staging already proved the entire interval. Keep pages bounded and publish
   * the durable cursor only after all writes and snapshot pruning succeed. */
  async function persistStagedEntityTransfer(
    interval: import("./entityTransfer.ts").StagedEntityInterval,
    isCurrent: () => boolean,
  ) {
    if (interval.resourceEntry) {
      await persistResourceEntityChanges(
        {
          protocol: interval.protocol,
          workspaceId: interval.workspaceId,
          epoch: interval.epoch,
          revision: interval.revision,
          reason: interval.reason,
          resources: [interval.resource],
          resourceVersions: [interval.resourceEntry],
        },
        isCurrent,
      );
      return;
    }
    const { db, workspaceId } = await syncDatabase();
    if (!isCurrent() || workspaceId !== interval.workspaceId) return;
    const scopes: Record<
      string,
      {
        collections: readonly string[];
        idField: string | null;
        agentField: string | null;
        includeTeamMembers: boolean;
      }
    > = SUBSCRIPTION_ENTITY_SCOPES;
    const resource = interval.resource;
    const scope = scopes[resource.kind];
    if (!scope) throw new Error("Unknown staged entity scope");
    const selector = resource as Record<string, unknown>;
    const selectedAgent = scope.agentField
      ? selector[scope.agentField]
      : undefined;
    const selectedAgents = new Set([selectedAgent]);
    await withEntityProjectionWrite(db.projections.database.name, async () => {
      const applied = await readAppliedSubscriptionCursor(resource);
      if (!isCurrent() || (applied && applied.seq >= interval.cursor.seq))
        return;
      if (
        interval.replyKind === SubscriptionReplyKind.delta &&
        (!applied || applied.seq < interval.after)
      )
        throw new Error("Staged subscription delta has no applied base");
      if (selectedAgent && scope.includeTeamMembers) {
        let after = "entity:agentSummary:";
        for (;;) {
          const summaries = await db.projections
            .find({
              selector: { id: { $gt: after, $lt: "entity:agentSummary;" } },
              limit: SYNC_ENTITY_STREAM_MAX_DOCUMENTS,
            })
            .exec();
          for (const doc of summaries) {
            const row = doc.toJSON();
            const entity = row.entity;
            if (
              entity?.collection === "agentSummary" &&
              entity.value.rootId === selectedAgent
            )
              selectedAgents.add(entity.id);
            after = row.id;
          }
          if (summaries.length < SYNC_ENTITY_STREAM_MAX_DOCUMENTS) break;
        }
      }
      const belongs = (entity: {
        collection: string;
        id: string;
        value?: unknown;
      }) => {
        if (!scope.collections.includes(entity.collection)) return false;
        if (scope.idField && entity.id !== selector[scope.idField])
          return false;
        if (selectedAgent && entity.value) {
          const value = entity.value as { agent?: string; rootId?: string };
          if (
            !(scope.includeTeamMembers && selectedAgents.has(value.agent)) &&
            value.rootId !== selectedAgent
          )
            return false;
        }
        return true;
      };
      // Verify every page's scope before applying the first page. These reads
      // touch temporary local staging only; the server source is never reread.
      for await (const page of interval.pages()) {
        if (!isCurrent()) return;
        if (page.documents.some((row) => !belongs(row)))
          throw new Error("Entity outside staged subscription scope");
      }
      for await (const page of interval.pages()) {
        if (!isCurrent()) return;
        await persistProjectionBatch(
          db.projections,
          page.documents.map(entityCacheRow),
          isCurrent,
        );
      }
      if (interval.replyKind === SubscriptionReplyKind.snapshot) {
        for (const collection of scope.collections) {
          let after = `entity:${collection}:`;
          for (;;) {
            if (!isCurrent()) return;
            const existing = await db.projections
              .find({
                selector: { id: { $gt: after, $lt: `entity:${collection};` } },
                limit: SYNC_ENTITY_STREAM_MAX_DOCUMENTS,
              })
              .exec();
            const removed: SyncDocument[] = [];
            for (const doc of existing) {
              const row = doc.toJSON();
              after = row.id;
              if (
                row.seq <= interval.cursor.seq &&
                row.entity &&
                belongs(row.entity) &&
                !(await interval.has(`${collection}:${row.entity.id}`))
              )
                removed.push({
                  id: row.id,
                  payload: "",
                  seq: row.seq,
                  _deleted: true,
                  evicted: true,
                });
            }
            await persistProjectionBatch(db.projections, removed, isCurrent);
            if (existing.length < SYNC_ENTITY_STREAM_MAX_DOCUMENTS) break;
          }
        }
      }
      if (!isCurrent()) return;
      if (resource.kind === "state") {
        await persistProjection(db.projections, {
          id: "state:entities:checkpoint",
          payload: "{}",
          seq: interval.cursor.seq,
        });
        if (interval.replyKind === SubscriptionReplyKind.snapshot) {
          const [ready] = await db.projections.getRows(
            ["state:entities:ready"],
            true,
          );
          await persistProjection(db.projections, {
            id: "state:entities:complete",
            payload: "complete",
            seq: Date.now(),
          });
          await persistProjection(db.projections, {
            id: "state:entities:ready",
            payload: "ready",
            seq: (ready?.seq ?? 0) + 1,
          });
        }
      }
      await persistSubscriptionCursor(resource, interval.cursor, isCurrent);
    });
    if (!isCurrent()) return;
    for await (const page of interval.pages()) {
      if (!isCurrent()) return;
      await freshReceipts(workspaceId, page.documents);
    }
    if (isCurrent()) await cacheSubscription(resource).catch(() => {});
  }

  async function persistResourceEntityChanges(
    event: import("./subscriptions/events.ts").ResourceChangeEvent,
    isCurrent: () => boolean,
  ) {
    // Hints participate in delivery ordering but have no durable projection data.
    if (
      !event.resourceVersions.some(
        (entry) =>
          entry.cursor ||
          entry.entityChanges ||
          entry.transcriptChanges ||
          entry.transcriptSnapshot,
      )
    )
      return;
    const { db, workspaceId } = await syncDatabase();
    if (!isCurrent() || workspaceId !== event.workspaceId) return;
    for (const [index, entry] of event.resourceVersions.entries()) {
      if (!isCurrent()) return;
      const resource = entry.resource ?? event.resources[index];
      if (
        resource &&
        entry.cursor &&
        entry.viewSnapshot &&
        (resource.kind === "terminal-output" || resource.kind === "room")
      ) {
        const cursor = entry.cursor;
        if (
          entry.replyKind !== SubscriptionReplyKind.snapshot &&
          entry.replyKind !== SubscriptionReplyKind.delta
        )
          throw new Error("View subscription has no reply kind");
        if (
          resource.kind === "room" &&
          entry.replyKind !== SubscriptionReplyKind.snapshot
        )
          throw new Error("Room subscription requires a complete snapshot");
        if (cursor.workspaceId !== workspaceId)
          throw new Error("Foreign view cursor");
        await withEntityProjectionWrite(
          db.projections.database.name,
          async () => {
            if (!isCurrent()) return;
            const id = await cursorDocumentId(resource);
            const [row] = await db.projections.getRows([id], false);
            const previous = row?.payload ? JSON.parse(row.payload) : undefined;
            if (
              resource.kind === "terminal-output" &&
              previous?.workspaceId === workspaceId &&
              previous.viewEpoch === event.epoch &&
              previous.viewRevision >= entry.revision
            ) {
              // Late snapshots must not restore output from before a reset.
              entry.cursor = {
                workspaceId,
                seq: previous.seq,
                terminalGeneration: previous.terminalGeneration,
              };
              entry.viewSnapshot = previous.viewSnapshot;
              return;
            }
            const snapshot = { ...entry.viewSnapshot };
            if (
              snapshot.terminalOutput &&
              entry.replyKind === SubscriptionReplyKind.delta &&
              (previous?.workspaceId !== workspaceId ||
                cursor.terminalGeneration === undefined ||
                cursor.terminalGeneration !== previous.terminalGeneration ||
                entry.terminalBaseOffset === undefined ||
                entry.terminalBaseOffset !==
                  previous.viewSnapshot?.terminalOutput?.offset)
            )
              throw new Error("Terminal output cursor has no local base");
            if (snapshot.terminalOutput)
              snapshot.terminalOutput = mergeTerminalOutput(
                previous?.workspaceId === workspaceId
                  ? previous.viewSnapshot?.terminalOutput
                  : undefined,
                snapshot.terminalOutput,
                entry.replyKind === SubscriptionReplyKind.delta,
              );
            // A replayed terminal interval keeps the cursor of the merged base.
            // Snapshots may reset the offset, and status can change without output.
            const appliedCursor = snapshot.terminalOutput
              ? { ...cursor, seq: snapshot.terminalOutput.offset }
              : cursor;
            if (!isCurrent()) return;
            await persistProjection(
              db.projections,
              {
                id,
                payload: JSON.stringify({
                  ...appliedCursor,
                  ...(resource.kind === "terminal-output"
                    ? { viewEpoch: event.epoch, viewRevision: entry.revision }
                    : {}),
                  viewSnapshot: snapshot,
                }),
                seq: appliedCursor.seq,
              },
              resource.kind === "terminal-output",
            );
            entry.cursor = appliedCursor;
            entry.viewSnapshot = snapshot;
          },
        );
        continue;
      }
      if (entry.entityChanges && !entry.cursor)
        throw new Error("Entity subscription has no cursor");
      if (
        resource &&
        entry.cursor &&
        entry.entityChanges &&
        (await persistScopedEntities(resource, entry, isCurrent))
      )
        continue;
      if (
        resource &&
        entry.cursor &&
        entry.replyKind === SubscriptionReplyKind.delta &&
        !entry.transcriptChanges &&
        !entry.transcriptSnapshot &&
        !entry.entityChanges
      ) {
        const applied = await readAppliedSubscriptionCursor(resource);
        if (!isCurrent()) return;
        if (!applied || applied.seq < entry.cursor.seq)
          throw new Error("Empty subscription delta has no applied local base");
        continue;
      }

      if (
        resource?.kind === "transcript" &&
        (entry.transcriptSnapshot || entry.transcriptChanges) &&
        (!entry.cursor ||
          entry.cursor.workspaceId !== workspaceId ||
          entry.cursor.seq !== entry.revision)
      )
        throw new Error("Invalid transcript subscription cursor");
      if (resource?.kind === "transcript" && entry.transcriptSnapshot) {
        await persistTranscriptProjection(
          db.projections,
          `transcript:${resource.agentId}`,
          {
            id: `transcript:${resource.agentId}`,
            payload: "",
            seq: entry.revision,
            _deleted: false,
          },
          workspaceId,
          entry.transcriptSnapshot,
          undefined,
          isCurrent,
          event.epoch,
          entry.cursor!,
        );
        continue;
      }
      if (resource?.kind === "transcript" && entry.transcriptChanges) {
        const batch = entry.transcriptChanges;
        if (
          batch.through !== entry.revision ||
          batch.after < 0 ||
          batch.through <= batch.after
        )
          throw new TypeError("Invalid stream transcript batch");
        await persistTranscriptProjection(
          db.projections,
          `transcript:${resource.agentId}`,
          {
            id: `transcript:${resource.agentId}`,
            payload: "",
            seq: batch.through,
            _deleted: false,
          },
          workspaceId,
          batch.delta,
          batch.after,
          isCurrent,
          event.epoch,
          entry.cursor!,
        );
        continue;
      }
    }
    if (!isCurrent()) return;
    for (const [index, entry] of event.resourceVersions.entries()) {
      if (!isCurrent()) return;
      if (entry.entityChanges)
        await freshReceipts(workspaceId, entry.entityChanges.documents);
      const resource = entry.resource ?? event.resources[index];
      if (entry.cursor && resource)
        await cacheSubscription(resource).catch(() => {});
    }
  }

  function resourceForScope(scope: string): ResourceRef {
    if (scope === "drafts") return { kind: "drafts" };
    if (scope.startsWith("transcript:"))
      return { kind: "transcript", agentId: scope.slice("transcript:".length) };
    return { kind: "state" };
  }

  function watchSyncInvalidations(
    resync: (version?: ResourceVersion) => void,
    scope?: string,
  ): () => void;

  function watchSyncInvalidations(
    scope: string,
    resync: (version?: ResourceVersion) => void,
  ): () => void;

  function watchSyncInvalidations(
    first: string | (() => void),
    second: string | (() => void) = "legacy",
  ) {
    const scope = typeof first === "string" ? first : String(second);
    const resync = typeof first === "function" ? first : second;
    if (typeof resync !== "function") return () => {};
    return watchResourceChanges(resourceForScope(scope), resync, {
      waitForBaseline: true,
    });
  }

  function watchTranscriptInvalidations(
    id: string,
    resync: (version?: ResourceVersion) => void,
    resource?: Extract<ResourceRef, { kind: "transcript" }>,
  ) {
    return watchResourceChanges(
      resource ?? { kind: "transcript", agentId: id },
      resync,
      { waitForBaseline: true },
    );
  }

  async function persistProjection(
    collection: LocalCollection,
    incoming: SyncDocument,
    replaceObsoleteTranscript = false,
  ) {
    await collection.applyBatch([incoming], {
      replace: replaceObsoleteTranscript,
    });
  }

  async function persistProjectionBatch(
    collection: LocalCollection,
    documents: SyncDocument[],
    isCurrent: () => boolean = () => true,
  ) {
    await collection.applyBatch(documents, { isCurrent });
  }

  const transcriptItemId = (scope: string, id: string) =>
    `transcript:item:${scope.slice("transcript:".length)}:${id}`;

  function isTranscriptDelta(
    value: TranscriptSyncPayload,
  ): value is components["schemas"]["TranscriptStreamDelta"] {
    return "delta" in value;
  }

  function transcriptMetadata(value: TranscriptSyncPayload) {
    if (isTranscriptDelta(value)) {
      const {
        items: _items,
        delta: _delta,
        removed: _removed,
        order: _order,
        ...metadata
      } = value;
      return metadata;
    }
    const { items: _items, ...metadata } = value;
    return metadata;
  }

  async function cacheStoredTranscript(
    collection: LocalCollection,
    workspaceId: string,
    scope: string,
    doc: SyncDocument,
  ) {
    const id = scope.slice("transcript:".length);
    if (doc._deleted) {
      cacheTranscriptValue(workspaceId, id, null, doc.seq, 0);
      return;
    }
    if ((peekTranscript(workspaceId, id)?.seq || 0) >= doc.seq) return;
    const meta = readTranscriptMeta(doc.payload);
    if (!meta) return;
    const rows = meta.order.length
      ? await collection.getRows(
          meta.order.map((itemId) => transcriptItemId(scope, itemId)),
          true,
        )
      : [];
    const byId = new Map(rows.map((row) => [row.id, row]));
    const items = meta.order.flatMap((itemId) => {
      const row = byId.get(transcriptItemId(scope, itemId));
      if (!row || row._deleted) return [];
      try {
        return [JSON.parse(row.payload)];
      } catch {
        return [];
      }
    });
    cacheTranscriptValue(workspaceId, id, { ...meta.metadata, items }, doc.seq);
  }

  /** A store subscriber can see a meta row before its writer updates the
   * cache. Check after that write settles, so a patched cache skips the read. */
  function cacheStoredTranscriptAfterWrite(
    collection: LocalCollection,
    workspaceId: string,
    scope: string,
    doc: SyncDocument,
  ) {
    setTimeout(
      () => void cacheStoredTranscript(collection, workspaceId, scope, doc),
      0,
    );
  }

  async function persistTranscriptProjection(
    collection: LocalCollection,
    scope: string,
    document: SyncDocument,
    workspaceId: string,
    incoming: TranscriptSyncPayload | null,
    after: number | undefined,
    isCurrent: () => boolean,
    epoch: string | undefined,
    cursor: components["schemas"]["SubscriptionCursor"],
  ) {
    return withEntityProjectionWrite(collection.database.name, async () => {
      if (!isCurrent()) return;
      const [previous] = await collection.getRows([scope], true);
      const base = previous?.payload
        ? readTranscriptMeta(previous.payload)
        : null;
      const durableResume = cursor.workspaceId === workspaceId;
      const epochChanged =
        !durableResume &&
        base?.epoch !== undefined &&
        epoch !== undefined &&
        base.epoch !== epoch;
      if (epochChanged && incoming !== null && isTranscriptDelta(incoming))
        throw new Error("Transcript delta belongs to a different epoch.");
      const cursorId = await cursorDocumentId({
        kind: "transcript",
        agentId: scope.slice("transcript:".length),
      });
      if (!epochChanged && previous && previous.seq >= document.seq && base) {
        const [applied] = await collection.getRows([cursorId], false);
        if (
          previous.seq > document.seq ||
          (incoming !== null &&
            isTranscriptDelta(incoming) &&
            parseAppliedCursor(applied?.payload, workspaceId)?.seq ===
              document.seq)
        )
          return;
      }
      if (after !== undefined && previous?.seq !== after)
        throw new Error("Transcript stream interval has no local base.");
      const id = scope.slice("transcript:".length);
      const oldOrder: string[] = base?.order || [];
      const delta =
        incoming !== null && isTranscriptDelta(incoming) ? incoming : null;
      const isDelta = delta !== null;
      if (isDelta && !base)
        throw new Error("Transcript delta has no local item base.");

      let changedItems = incoming?.items ?? [];
      // A window baseline omits older rows present in the shared pull projection.
      // Keep only a contiguous retained suffix when that projection sends its
      // complete order; an edit outside the window must not bridge a missing row.
      let windowOrder: string[] | undefined;
      let windowTruncated = false;
      if (delta && base?.metadata.truncated) {
        const available = new Set(
          oldOrder.filter((key) => !delta.removed.includes(key)),
        );
        if (delta.order) {
          for (const item of changedItems) available.add(item.id);
          let start = delta.order.length;
          for (const key of delta.order.toReversed()) {
            if (!available.has(key)) break;
            start--;
          }
          windowOrder = delta.order.slice(start);
          windowTruncated = start > 0;
        } else {
          windowOrder = oldOrder.filter((key) => available.has(key));
          windowTruncated = true;
        }
        const retained = new Set(windowOrder);
        changedItems = changedItems.filter((item) => retained.has(item.id));
      }
      const removed =
        incoming === null ? [...oldOrder] : delta ? [...delta.removed] : [];
      const snapshotIds = changedItems.map((item) => item.id);
      const order =
        incoming === null
          ? []
          : windowOrder
            ? windowOrder
            : delta?.order
              ? delta.order
              : isDelta
                ? oldOrder
                    .filter((key) => !removed.includes(key))
                    .concat(
                      changedItems
                        .map((item) => item.id)
                        .filter((key: string) => !oldOrder.includes(key)),
                    )
                : snapshotIds;
      if (!document._deleted && !isDelta)
        removed.push(...oldOrder.filter((key) => !order.includes(key)));
      const previousMetadata = isDelta ? base?.metadata || {} : {};
      const metadata: TranscriptProjectionMeta["metadata"] =
        incoming === null
          ? {}
          : { ...previousMetadata, ...transcriptMetadata(incoming) };
      if (windowTruncated) {
        metadata.truncated = true;
        metadata.nextCursor =
          order[0] === oldOrder[0]
            ? (base?.metadata.nextCursor ?? order[0] ?? null)
            : (order[0] ?? base?.metadata.nextCursor ?? null);
      }
      const nextMeta: TranscriptProjectionMeta = {
        format: TRANSCRIPT_PROJECTION_FORMAT,
        epoch: epoch ?? base?.epoch,
        metadata,
        order,
      };
      const rows: SyncDocument[] = changedItems.map((item) => ({
        id: transcriptItemId(scope, item.id),
        payload: JSON.stringify(item),
        seq: document.seq,
      }));
      rows.push(
        ...removed.map((itemId) => ({
          id: transcriptItemId(scope, itemId),
          payload: "{}",
          seq: document.seq,
          _deleted: true,
        })),
      );
      const metaDocument: SyncDocument = {
        id: scope,
        payload: JSON.stringify(nextMeta),
        seq: document.seq,
        _deleted: document._deleted,
      };
      // Publish data, metadata and its receipt before any observer can cancel.
      const committed = await collection.applyBatch(
        [
          ...rows,
          metaDocument,
          {
            id: cursorId,
            payload: JSON.stringify(cursor),
            seq: cursor.seq,
          },
        ],
        {
          isCurrent,
          replace:
            epochChanged ||
            (!isDelta && previous?.seq === document.seq) ||
            (!document._deleted && !isDelta && !!previous && !base),
        },
      );
      if (!committed) return;
      if (epochChanged || !isDelta) clearTranscript(workspaceId, id);
      if (incoming !== null && !isDelta) {
        cacheTranscriptValue(
          workspaceId,
          id,
          { ...metadata, items: changedItems },
          document.seq,
          document.payload.length * 2,
        );
      } else if (incoming !== null) {
        const cacheMetadata = metadata;
        // Patch only a cache entry at this delta's durable base. A write that
        // stopped after its durable update can leave the cache behind it.
        const patched =
          peekTranscript(workspaceId, id)?.seq === previous?.seq &&
          patchTranscriptValue(
            workspaceId,
            id,
            cacheMetadata,
            document.seq,
            changedItems,
            removed,
            delta?.order || windowOrder ? order : undefined,
          );
        if (!patched)
          await cacheStoredTranscript(
            collection,
            workspaceId,
            scope,
            metaDocument,
          );
      } else {
        cacheTranscriptValue(workspaceId, id, null, document.seq, 0);
      }
    });
  }

  async function watchTranscriptProjection(
    scope: `transcript:${string}`,
    accept: (payload: TranscriptProjectionPayload) => void,
    fail: (e: unknown) => void,
    localOnly = false,
    resource?: Extract<ResourceRef, { kind: "transcript" }>,
  ) {
    const { db, workspaceId } = await syncDatabase();
    const release =
      localOnly && !resource
        ? () => {}
        : watchTranscriptInvalidations(
            scope.slice("transcript:".length),
            (version) => {
              if (version?.cursor) fail(null);
              else if (!options.api.isApiSchemaMismatch())
                options.resources.replaceStream?.();
            },
            resource,
          );
    void subscriptionCache?.load(
      resource ?? {
        kind: "transcript",
        agentId: scope.slice("transcript:".length),
      },
    );
    const id = scope.slice("transcript:".length);
    const stopCache = subscribeTranscript(workspaceId, id, (entry) =>
      accept(entry.payload),
    );
    const subscription = db.projections.findOne(scope).$.subscribe((doc) => {
      if (doc)
        cacheStoredTranscriptAfterWrite(
          db.projections,
          workspaceId,
          scope,
          doc,
        );
      else if (!peekTranscript(workspaceId, id)) accept(null);
    });
    return () => {
      subscription.unsubscribe();
      stopCache();
      void release();
    };
  }

  /** User-requested refresh uses the same durable SSE resume path. */
  async function refreshState() {
    let live = false;
    const stop = watchResourceConnection((status) => {
      live = status === "live";
    });
    stop();
    if (!live && !options.api.isApiSchemaMismatch())
      options.resources.replaceStream?.();
  }

  /** Attach local observation once; the transport owns reconnection. */
  function subscribeProjection<T>(
    watch: (
      accept: (payload: T) => void,
      fail: (error: unknown) => void,
    ) => Promise<() => void>,
    accept: (payload: T) => void,
    report: (error: unknown | null) => void,
  ) {
    let stopped = false;
    let attaching = false;
    let attached = false;
    let dispose = () => {};
    const connect = async () => {
      if (stopped || attaching || attached) return;
      attaching = true;
      try {
        const stop = await watch(
          (value) => {
            if (!stopped) accept(value);
          },
          (error) => {
            if (!stopped) report(error);
          },
        );
        if (stopped) stop();
        else {
          dispose = stop;
          attached = true;
        }
      } catch (error) {
        if (!stopped) report(error);
      } finally {
        attaching = false;
      }
    };
    const stopResume = onResume(() => void connect());
    const stopConnection = watchResourceConnection((status) => {
      if (status === "live") void connect();
    });
    void connect();
    return () => {
      stopped = true;
      stopResume();
      stopConnection();
      dispose();
    };
  }

  /** Own the durable state delivery engine while its generated observer is present. */
  function retainStateDelivery(report: (error: unknown | null) => void) {
    return subscribeProjection<void>(
      async (accept, fail) => {
        const { db } = await syncDatabase();
        const release = watchSyncInvalidations((version) => {
          if (version?.cursor && version.replyKind) fail(null);
          else if (!options.api.isApiSchemaMismatch())
            options.resources.replaceStream?.();
        }, "entities");
        let frame = 0;
        const cacheAfterApply = () => {
          if (
            typeof requestAnimationFrame !== "function" ||
            !metadata.current.ready
          )
            return;
          cancelAnimationFrame(frame);
          frame = requestAnimationFrame(() => {
            frame = requestAnimationFrame(() => {
              void cacheSubscription({ kind: "state" }).catch(() => {});
            });
          });
        };
        const cacheChanges = db.projections.$.subscribe(({ documentData }) => {
          if (
            documentData.id.startsWith("entity:") ||
            documentData.id === "state:entities:complete"
          )
            cacheAfterApply();
        });
        const stopReady = autorun(() => {
          if (metadata.current.ready) {
            accept();
            cacheAfterApply();
          }
        });
        return () => {
          stopReady();
          cacheChanges.unsubscribe();
          if (typeof cancelAnimationFrame === "function")
            cancelAnimationFrame(frame);
          void release();
        };
      },
      () => {},
      report,
    );
  }

  function subscribeTranscriptProjection(
    scope: `transcript:${string}`,
    accept: (payload: TranscriptProjectionPayload) => void,
    report: (error: unknown | null) => void,
    localOnly = false,
    resource?: Extract<ResourceRef, { kind: "transcript" }>,
  ) {
    return subscribeProjection(
      (next, fail) =>
        watchTranscriptProjection(scope, next, fail, localOnly, resource),
      accept,
      report,
    );
  }
  return {
    get projectionMetadata() {
      return metadata.current;
    },
    refreshState,
    retainStateDelivery,
    syncDatabase,
    cacheSubscription,
    setCachePolicies(
      decisions: components["schemas"]["SubscriptionCacheDecision"][],
    ) {
      const session = cachePolicySession;
      const apply = () => {
        if (session !== cachePolicySession) return;
        for (const decision of decisions)
          void subscriptionCache
            ?.setPolicy(decision.resource, decision.policy)
            .catch(() => {});
      };
      if (subscriptionCache) apply();
      else
        void syncDatabase()
          .then(apply)
          .catch(() => {});
    },
    clearCachePolicies() {
      cachePolicySession = {};
      subscriptionCache?.clearPolicies();
    },
    disposeCache: () => subscriptionCache?.stop(),
    persistResourceEntityChanges,
    persistStagedEntityTransfer,
    readSubscriptionCursor,
    watchSyncInvalidations,
    persistProjection,
    subscribeTranscriptProjection,
  };
}
export const defaultSyncClient = createSyncClient({
  api: {
    syncGet,
    syncPost,
    saved,
    save,
    setWorkspace,
    isApiSchemaMismatch,
  },
  resources: {
    watchResourceChanges: (...args: Parameters<typeof watchResourceChanges>) =>
      watchResourceChanges(...args),
    watchResourceConnection: (
      ...args: Parameters<typeof watchResourceConnection>
    ) => watchResourceConnection(...args),
    replaceStream: () => replaceStream(),
  },
  sequences: { getEntitySequenceCheckpoint, setEntitySequenceProjection },
  transcripts: {
    clearTranscript,
    cacheTranscriptValue,
    peekTranscript,
    patchTranscriptValue,
    subscribeTranscript,
  },
  databaseSuffix: serverDatabaseSuffix,
});
export const {
  projectionMetadata,
  refreshState,
  retainStateDelivery,
  setCachePolicies,
  clearCachePolicies,
  syncDatabase,
  persistResourceEntityChanges,
  persistStagedEntityTransfer,
  readSubscriptionCursor,
  watchSyncInvalidations,
  persistProjection,
  subscribeTranscriptProjection,
} = defaultSyncClient;

import { compareStructural } from "mobx";
import type { SyncEntityPayload, ModelEntityDto } from "../generated/api";
import type { components } from "../generated/api";
export type UiPreferencesDto = components["schemas"]["UiPreferencesDto"];
import type { Snapshot } from "../types";

export const UI_PREFERENCE_SCOPES = ["user", "server"] as const;
export type UiPreferenceScope = (typeof UI_PREFERENCE_SCOPES)[number];
export const UI_PREFERENCE_ENTITY_IDS = UI_PREFERENCE_SCOPES.map(
  (scope) => `entity:uiPreferences:${scope}`,
);
export type UiPreferenceProjection = Partial<
  Record<UiPreferenceScope, UiPreferencesDto>
>;

/** Local cache rows retain structured entities; storage fills the tombstone flag. */
export type EntityRow = Omit<
  components["schemas"]["SyncDocument"],
  "_deleted"
> & {
  _deleted?: components["schemas"]["SyncDocument"]["_deleted"] | undefined;
  entity?: SyncEntityPayload;
  /** Local selection eviction; seq remains the last authoritative source version. */
  evicted?: boolean;
};

import {
  ENTITY_COLLECTIONS,
  emptyEntityValues,
  setEntityValue,
} from "../generated/entityCollections.ts";
import type {
  EntityCollection,
  EntityFor,
  EntityValues,
} from "../generated/entityCollections.ts";
const entityCollections = new Set<string>(ENTITY_COLLECTIONS);
type MergedAgentCache = WeakMap<
  EntityFor<"agentSummary">,
  {
    detail: EntityFor<"agentDetail"> | undefined;
    value: Snapshot["threads"][number];
  }
>;
type EntityRowState = {
  seq: number;
  collection?: EntityCollection;
  entityId?: string;
  deleted?: boolean;
  evicted?: boolean;
};

export type EntityProjection = {
  rows: Map<string, EntityRowState>;
  invalidReportedSeq: Map<string, number>;
  tombstones: Set<string>;
  changed: Set<EntityCollection>;
  values: EntityValues;
  snapshot: Snapshot | null;
  agentSummarySeen: boolean;
  mergedAgents: MergedAgentCache;
};

/**
 * The schema-versioned cache contains payloads from this renderer contract.
 * Shape validation remains necessary because a mismatched server response or
 * storage corruption can still damage a row before it reaches the cache.
 */
export function parseEntityPayload(parsed: unknown): SyncEntityPayload | null {
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed))
    return null;
  const candidate = parsed as Record<string, unknown>;
  if (
    typeof candidate.collection !== "string" ||
    !entityCollections.has(candidate.collection) ||
    typeof candidate.id !== "string" ||
    candidate.id.length === 0 ||
    candidate.value === null ||
    typeof candidate.value !== "object" ||
    Array.isArray(candidate.value)
  )
    return null;
  return candidate as SyncEntityPayload;
}

function entityValues<C extends EntityCollection>(
  values: EntityValues,
  collection: C,
): EntityFor<C>[] {
  return [...values[collection].values()];
}

function projectSnapshotCollections(
  prior: Snapshot | null,
  changed: ReadonlySet<EntityCollection>,
  values: EntityValues,
  agentSummarySeen: boolean,
  mergedAgents: MergedAgentCache,
): Snapshot {
  const agents =
    prior &&
    !changed.has("agent") &&
    !changed.has("agentSummary") &&
    !changed.has("agentDetail")
      ? prior.runtime.agents
      : agentSummarySeen
        ? entityValues(values, "agentSummary").map((summary) => {
            const detail = values.agentDetail.get(summary.id);
            const cached = mergedAgents.get(summary);
            if (cached && cached.detail === detail) return cached.value;
            const value = { ...detail, ...summary };
            mergedAgents.set(summary, { detail, value });
            return value;
          })
        : entityValues(values, "agent");
  const chats =
    prior && !changed.has("chat") ? prior.chats : entityValues(values, "chat");
  const edges =
    prior && !changed.has("edge") ? prior.edges : entityValues(values, "edge");
  const rooms =
    prior && !changed.has("room")
      ? prior.runtime.rooms
      : entityValues(values, "room");
  const tasks =
    prior && !changed.has("task")
      ? prior.runtime.tasks
      : entityValues(values, "task");
  const monitors =
    prior && !changed.has("monitor")
      ? prior.runtime.monitors
      : entityValues(values, "monitor");
  const complaints =
    prior && !changed.has("complaint")
      ? prior.runtime.complaints
      : entityValues(values, "complaint");
  const requests =
    prior && !changed.has("request")
      ? prior.runtime.requests
      : entityValues(values, "request");
  const rules =
    prior && !changed.has("rule")
      ? prior.runtime.rules
      : entityValues(values, "rule");
  // Projects of paired servers carry their server's name. The server publishes them for
  // clients that cannot reach those servers; they are not local projects.
  const remoteProjects =
    prior && !changed.has("project")
      ? prior.runtime.remoteProjects
      : entityValues(values, "project").filter(
          (project) => project.serverLabel,
        );
  const projects =
    prior && !changed.has("project")
      ? prior.runtime.projects
      : entityValues(values, "project").filter(
          (project) => !project.serverLabel,
        );
  const peerTeams =
    prior && !changed.has("peerTeam")
      ? prior.runtime.peerTeams
      : entityValues(values, "peerTeam");
  const events =
    prior && !changed.has("event")
      ? prior.runtime.events
      : entityValues(values, "event");
  const work =
    prior && !changed.has("work")
      ? prior.runtime.work
      : entityValues(values, "work");
  const workspace = values.workspace.get("current");
  const rateLimitsByAccount =
    prior && !changed.has("rateLimit")
      ? prior.runtime.rateLimitsByAccount
      : Object.fromEntries(values.rateLimit);
  let modelsByAccount: Record<string, ModelEntityDto[]>;
  if (prior && !changed.has("model")) {
    modelsByAccount = prior.runtime.modelsByAccount ?? {};
  } else {
    modelsByAccount = {};
    for (const model of entityValues(values, "model")) {
      (modelsByAccount[model.accountKey] ??= []).push(model);
    }
    for (const models of Object.values(modelsByAccount))
      models.sort(
        (left, right) => left.catalogPosition - right.catalogPosition,
      );
  }
  const studioErrors =
    prior && !changed.has("studioError")
      ? prior.runtime.studioErrors
      : entityValues(values, "studioError").sort(
          (a, b) => b.at - a.at || a.id.localeCompare(b.id),
        );
  const runtime = {
    ...workspace,
    modelsByAccount,
    retainedWorktrees:
      prior && !changed.has("retainedWorktree")
        ? prior.runtime.retainedWorktrees
        : entityValues(values, "retainedWorktree"),
    questionHistory:
      prior && !changed.has("questionHistory")
        ? prior.runtime.questionHistory
        : entityValues(values, "questionHistory"),
    plans:
      prior && !changed.has("plan")
        ? prior.runtime.plans
        : entityValues(values, "plan"),
    annotations:
      prior && !changed.has("annotation")
        ? prior.runtime.annotations
        : entityValues(values, "annotation"),
    accountsSummary:
      prior && !changed.has("accounts")
        ? prior.runtime.accountsSummary
        : values.accounts.get("current"),
    accountCostsByAccount:
      prior && !changed.has("accountCost")
        ? prior.runtime.accountCostsByAccount
        : Object.fromEntries(values.accountCost),
    sessionCostsByAgent:
      prior && !changed.has("sessionCost")
        ? prior.runtime.sessionCostsByAgent
        : Object.fromEntries(values.sessionCost),
    desktop:
      prior && !changed.has("desktop")
        ? prior.runtime.desktop
        : values.desktop.get("current"),
    rateLimitsByAccount,
    rateLimits: rateLimitsByAccount?.default ?? null,
    studioErrors,
    agents,
    rooms,
    tasks,
    monitors,
    complaints,
    requests,
    rules,
    projects,
    remoteProjects,
    peerTeams,
    events,
    work,
  };

  return {
    ...prior,
    token: "",
    stateDir: workspace?.stateDir ?? prior?.stateDir ?? "",
    threads: agents,
    chats,
    nodes:
      prior &&
      !changed.has("agent") &&
      !changed.has("agentSummary") &&
      !changed.has("agentDetail") &&
      !changed.has("chat")
        ? prior.nodes
        : [...agents, ...chats],
    edges,
    runtime,
  };
}

export function emptyEntityProjection(): EntityProjection {
  return {
    rows: new Map(),
    invalidReportedSeq: new Map(),
    tombstones: new Set(),
    changed: new Set(),
    values: emptyEntityValues(),
    snapshot: null,
    agentSummarySeen: false,
    // Unchanged agents keep their identity when another agent streams a change.
    // Weak keys release entries once their summary leaves the projection.
    mergedAgents: new WeakMap(),
  };
}

function retainTombstone(
  state: EntityProjection,
  rowId: string,
  row: EntityRowState,
) {
  state.rows.delete(rowId);
  state.rows.set(rowId, row);
  state.tombstones.delete(rowId);
  state.tombstones.add(rowId);
  // Match full-query retention without scanning the live rows after each delete.
  if (state.tombstones.size > 4096) {
    state.tombstones.delete(rowId);
    state.rows.delete(rowId);
  }
}

/** Apply explicit changes by ID. Absent rows remain present until a tombstone
 * arrives. Passing ready=false retains changes for one later publication. */
type EntityProjectionRow = Readonly<
  Pick<EntityRow, "id" | "seq" | "_deleted" | "evicted">
> & { readonly entity?: unknown };

export function applyEntityChanges(
  state: EntityProjection,
  rows: Iterable<EntityProjectionRow>,
  ready: true,
): Snapshot;
export function applyEntityChanges(
  state: EntityProjection,
  rows: Iterable<EntityProjectionRow>,
  ready: boolean,
): Snapshot | null;
export function applyEntityChanges(
  state: EntityProjection,
  rows: Iterable<EntityProjectionRow>,
  ready: boolean,
): Snapshot | null {
  const changed = state.changed;
  for (const row of rows) {
    if (!row.id.startsWith("entity:")) continue;
    const previous = state.rows.get(row.id);
    if (
      previous &&
      (previous.seq > row.seq ||
        (previous.seq === row.seq &&
          (row.evicted
            ? previous.deleted
            : !previous.evicted && (!row._deleted || previous.deleted))))
    )
      continue;
    const oldCollection = previous?.collection;
    const oldEntityId = previous?.entityId;
    if (row._deleted) {
      state.invalidReportedSeq.delete(row.id);
      if (oldCollection && oldEntityId !== undefined) {
        state.values[oldCollection].delete(oldEntityId);
        changed.add(oldCollection);
      }
      retainTombstone(state, row.id, {
        seq: row.seq,
        ...(oldCollection ? { collection: oldCollection } : {}),
        ...(oldEntityId === undefined ? {} : { entityId: oldEntityId }),
        deleted: true,
        ...(row.evicted ? { evicted: true } : {}),
      });
      continue;
    }
    const payload = parseEntityPayload(row.entity);
    if (!payload) {
      if (state.invalidReportedSeq.get(row.id) !== row.seq) {
        state.invalidReportedSeq.set(row.id, row.seq);
        console.error(
          `Skipping sync entity row with invalid payload for document "${row.id}"`,
          { id: row.id, seq: row.seq },
        );
      }
      continue;
    }
    state.invalidReportedSeq.delete(row.id);
    if (
      oldCollection &&
      oldEntityId !== undefined &&
      (oldCollection !== payload.collection || oldEntityId !== payload.id)
    ) {
      state.values[oldCollection].delete(oldEntityId);
      changed.add(oldCollection);
    }
    // Read-time entities can be re-emitted for a dependent source change.
    // Keep equal facts stable, but still reject any later stale replay.
    if (
      compareStructural(
        state.values[payload.collection].get(payload.id),
        payload.value,
      )
    ) {
      state.rows.set(row.id, {
        seq: row.seq,
        collection: payload.collection,
        entityId: payload.id,
      });
      state.tombstones.delete(row.id);
      // Project sequences are also command proofs, even when its fields are equal.
      if (payload.collection === "project") changed.add("project");
      continue;
    }
    // Scoped details and tails remain owned by their generated facades.
    if (
      payload.collection === "taskDetail" ||
      payload.collection === "agentTail"
    )
      continue;
    if (payload.collection === "agentSummary") state.agentSummarySeen = true;
    setEntityValue(state.values, payload);
    state.rows.set(row.id, {
      seq: row.seq,
      collection: payload.collection,
      entityId: payload.id,
    });
    state.tombstones.delete(row.id);
    changed.add(payload.collection);
  }
  if (!ready) return null;
  const prior = state.snapshot;
  if (prior && !changed.size) return prior;
  const next = projectSnapshotCollections(
    prior,
    changed,
    state.values,
    state.agentSummarySeen,
    state.mergedAgents,
  );
  if (!prior || changed.has("project")) {
    next.runtime.projectSequences = Object.fromEntries(
      [...state.rows.values()]
        .filter(
          (row) =>
            row.collection === "project" &&
            !row.deleted &&
            row.entityId !== undefined,
        )
        .map((row) => [row.entityId!, row.seq]),
    );
  } else next.runtime.projectSequences = prior.runtime.projectSequences;
  changed.clear();
  state.snapshot = next;
  return next;
}

/** Reconcile a complete store query, including rows omitted after a delete. */
export function applyEntityRows(
  state: EntityProjection,
  rows: readonly EntityProjectionRow[],
  ready: true,
): Snapshot;
export function applyEntityRows(
  state: EntityProjection,
  rows: readonly EntityProjectionRow[],
  ready: boolean,
): Snapshot | null;
export function applyEntityRows(
  state: EntityProjection,
  rows: readonly EntityProjectionRow[],
  ready: boolean,
): Snapshot | null {
  applyEntityChanges(state, rows, false);
  const seen = new Set(rows.map((row) => row.id));
  for (const rowId of state.invalidReportedSeq.keys()) {
    if (!seen.has(rowId)) state.invalidReportedSeq.delete(rowId);
  }
  // Store find queries omit tombstones. Missing IDs therefore represent deletes.
  const missing: Array<[string, EntityRowState]> = [];
  for (const [rowId, previous] of state.rows) {
    if (seen.has(rowId)) continue;
    if (previous.deleted) continue;
    missing.push([rowId, previous]);
  }
  for (const [rowId, previous] of missing) {
    state.invalidReportedSeq.delete(rowId);
    if (previous.collection && previous.entityId !== undefined) {
      state.values[previous.collection].delete(previous.entityId);
      state.changed.add(previous.collection);
    }
    retainTombstone(state, rowId, { ...previous, deleted: true });
  }
  return applyEntityChanges(state, [], ready);
}

import type { DeepReadonly } from "../deepReadonly.ts";
import type { SyncEntityPayload, components } from "../generated/api.ts";
import { ENTITY_COLLECTIONS } from "../generated/entityCollections.ts";
export type UiPreferencesDto = components["schemas"]["UiPreferencesDto"];
export const UI_PREFERENCE_SCOPES = ["user", "server"] as const;
export type UiPreferenceScope = (typeof UI_PREFERENCE_SCOPES)[number];
export const UI_PREFERENCE_ENTITY_IDS = UI_PREFERENCE_SCOPES.map(
  (scope) => `entity:uiPreferences:${scope}`,
);
export type UiPreferenceProjection = Partial<
  Record<UiPreferenceScope, DeepReadonly<UiPreferencesDto> | undefined>
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

const entityCollections = new Set<string>(ENTITY_COLLECTIONS);
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

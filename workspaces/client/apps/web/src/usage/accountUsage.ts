import type { JsonInput, JsonInputOf, Json } from "../types";
import type { ApiError, GetResult } from "../api";
import { ACCOUNT_LIMITS_WINDOW_FRESH_SECONDS } from "../generated/sharedValues";

export type JsonObject = Json;

type UsageLimitsResponse = JsonInputOf<GetResult<"/api/limits">>;
type ClientLimitMetadata = { loading?: boolean; stale?: boolean };
export type AccountLimitsReadError = Pick<ApiError, "message" | "status"> & {
  location?: string[];
};
export type AccountLimitsSnapshot = UsageLimitsResponse &
  ClientLimitMetadata & { readError?: AccountLimitsReadError };

export function limitsReadSucceeded(
  snapshot: Pick<AccountLimitsSnapshot, "error">,
): boolean {
  return !snapshot.error;
}

export function limitsHydrationSucceeded(
  snapshot: Pick<AccountLimitsSnapshot, "data" | "error"> | null,
): boolean {
  return !!snapshot?.data && limitsReadSucceeded(snapshot);
}

export function jsonObject(
  value: JsonInput | null | undefined,
): JsonObject | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value
    : null;
}

export function accountLimits(
  value: AccountLimitsSnapshot | null | undefined,
  accountKey: string,
  accountId?: string | null,
): AccountLimitsSnapshot | null {
  if (!value || value.accountKey !== accountKey) return null;
  const limits = value;
  const data = limits.data;
  if (accountId && data?.accountId != null && data.accountId !== accountId)
    return null;
  return limits;
}

export function shouldReplaceLimitsSnapshot(
  current: AccountLimitsSnapshot | null | undefined,
  next: AccountLimitsSnapshot,
): boolean {
  if (next.data === null && limitsReadSucceeded(next)) return true;
  return !current || (current.at || 0) <= (next.at || 0);
}

export function limitsSnapshotIsFresh(
  value: AccountLimitsSnapshot | null | undefined,
  accountKey: string,
  accountId?: string | null,
  now = Date.now() / 1000,
): boolean {
  const snapshot = accountLimits(value, accountKey, accountId);
  return !!(
    snapshot &&
    limitsReadSucceeded(snapshot) &&
    typeof snapshot.at === "number" &&
    Number.isFinite(snapshot.at) &&
    now - snapshot.at < ACCOUNT_LIMITS_WINDOW_FRESH_SECONDS
  );
}

// Each server owns its in-memory snapshots and listeners.
export function createAccountLimitsCache(
  _storage?: Pick<Storage, "getItem" | "setItem">,
) {
  let cacheScope: string | undefined;
  let cache: Record<string, AccountLimitsSnapshot> = {};
  const cacheListeners = new Set<() => void>();
  function configureLimitsCache(scope: string) {
    if (scope === cacheScope) return;
    cacheScope = scope;
    cache = {};
  }
  function cachedAccountLimits(key: string, accountId?: string | null) {
    return accountLimits(cache[key], key, accountId);
  }
  function subscribeLimitsCache(listener: () => void) {
    cacheListeners.add(listener);
    return () => {
      cacheListeners.delete(listener);
    };
  }
  function cacheAccountLimits(snapshot: AccountLimitsSnapshot) {
    if (cacheScope === undefined) configureLimitsCache("");
    const previous = cache[snapshot.accountKey];
    if (previous === snapshot) return;
    if (!shouldReplaceLimitsSnapshot(previous, snapshot)) return;
    cache = { ...cache, [snapshot.accountKey]: snapshot };
    for (const listener of cacheListeners) listener();
  }
  function cachedLimitsByAccount() {
    return cache;
  }

  return {
    configureLimitsCache,
    cachedAccountLimits,
    subscribeLimitsCache,
    cacheAccountLimits,
    cachedLimitsByAccount,
  };
}

export const {
  configureLimitsCache,
  cachedAccountLimits,
  subscribeLimitsCache,
  cacheAccountLimits,
  cachedLimitsByAccount,
} = createAccountLimitsCache();

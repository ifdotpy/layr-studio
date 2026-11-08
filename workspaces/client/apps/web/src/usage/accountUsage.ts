import type { JsonInput, JsonInputOf, Json } from "../types.ts";
import type { ApiError, GetResult } from "../api.ts";
import type { DeepReadonly } from "../deepReadonly.ts";

export type JsonObject = Json;

type UsageLimitsResponse = JsonInputOf<GetResult<"/api/limits">>;
type ClientLimitMetadata = { loading?: boolean; stale?: boolean };
export type AccountLimitsReadError = Pick<ApiError, "message" | "status"> & {
  location?: string[];
};
export type AccountLimitsSnapshot = UsageLimitsResponse &
  ClientLimitMetadata & { readError?: AccountLimitsReadError };
export type AccountLimitsData = DeepReadonly<AccountLimitsSnapshot>;

export function limitsReadSucceeded(
  snapshot: Pick<AccountLimitsSnapshot, "error">,
): boolean {
  return !snapshot.error;
}

export function jsonObject(
  value: JsonInput | null | undefined,
): JsonObject | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value
    : null;
}

export function accountLimits(
  value: AccountLimitsData | null | undefined,
  accountKey: string,
  accountId?: string | null,
): AccountLimitsData | null {
  if (!value || value.accountKey !== accountKey) return null;
  if (accountId && value.data?.accountId && value.data.accountId !== accountId)
    return null;
  return value;
}

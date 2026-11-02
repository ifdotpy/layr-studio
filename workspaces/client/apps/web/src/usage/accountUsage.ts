import type { JsonInput, JsonInputOf, Json } from "../types";
import type { ApiError, GetResult } from "../api";

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
  _accountId?: string | null,
): AccountLimitsSnapshot | null {
  if (!value || value.accountKey !== accountKey) return null;
  return value;
}

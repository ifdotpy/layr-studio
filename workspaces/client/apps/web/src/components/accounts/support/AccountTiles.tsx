import type { DeepReadonly } from "../../../deepReadonly.ts";
import { resourceData } from "../../../store/runtime.ts";
import { LimitColor } from "../../../generated/api.ts";
import type { components } from "../../../generated/api.ts";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  accountLimits,
  type AccountLimitsData,
} from "../../../usage/accountUsage.ts";
import { formatPercent } from "../../conversation/Usage/Usage.tsx";
import type { Account } from "./Accounts.tsx";

/** "33% left": the one text form of a remaining limit. */
export const remainingText = (remaining: number) =>
  `${formatPercent(remaining)} left`;

type LimitAccount = Pick<Account, "id" | "accountId" | "status">;
export type { LimitAccount };

/** What one limits reading of one account shows. A provider limit belongs to
 * the provider account, so every view of an account reads it through here. */
type AccountProjection = DeepReadonly<components["schemas"]["RateLimitView"]>;

export function accountLimitView(
  account: LimitAccount,
  limits: AccountLimitsData | null,
  _signedInOnly = true,
  projection: AccountProjection | null = null,
) {
  const matching = accountLimits(limits, account.id, account.accountId);
  const error = matching?.data ? null : matching?.error || null;
  const matchingProjection =
    matching && projection ? projection.projection : null;
  const state = matchingProjection?.buckets.current;
  return {
    limits: matching,
    buckets: state?.buckets ?? [],
    reset: matchingProjection?.reset ?? null,
    error,
    reading: !matching || matchingProjection?.status.kind === "pending",
    remaining: !error ? (state?.remaining ?? null) : null,
    color: !error ? (state?.color ?? LimitColor.paused) : LimitColor.paused,
  };
}

export function useAccountLimitView(
  account: LimitAccount,
  { signedInOnly = true } = {},
) {
  const store = useServerStore();
  const projection =
    resourceData(store.state().current)?.rateLimitsByAccount.get(account.id) ??
    null;
  const limits = projection?.observation ?? null;
  return accountLimitView(account, limits, signedInOnly, projection);
}

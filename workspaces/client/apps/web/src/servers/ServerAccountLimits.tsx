import { resourceData } from "../store/runtime.ts";
import type { AccountLimitsData } from "../usage/accountUsage.ts";
import type { Account } from "../components/accounts/support/Accounts.tsx";
import type { ServerAccount } from "./navigation.ts";
import type { ServerServices } from "./serverServices.ts";

/** The server's own account that a panel row stands for. */
export const matchServerAccount = (
  accounts: readonly Account[] | undefined,
  row: Pick<ServerAccount, "provider" | "email" | "label" | "accountKey">,
) => {
  const matches = accounts?.filter(
    (account) =>
      account.provider === row.provider &&
      (row.accountKey !== undefined
        ? account.id === row.accountKey
        : row.email
          ? account.email?.toLocaleLowerCase() === row.email.toLocaleLowerCase()
          : account.label === row.label),
  );
  return matches?.length === 1 ? matches[0] : undefined;
};

/** One server's copy of a provider account. */
export type AccountSource = {
  serverId: string;
  serverLabel: string;
  account: Account;
  services?: ServerServices | undefined;
};

/** The reading that the account card shows. Usage limits belong to the
 * provider account, so every server reads the same limit: the card shows the
 * reading with data whose `at` (epoch seconds) is newest. Without any data it
 * shows a read in progress, then a read error, then an answer without data. */
export function newestReading<T extends { limits: AccountLimitsData | null }>(
  readings: T[],
): T | undefined {
  let newest: T | undefined;
  for (const reading of readings)
    if (
      reading.limits?.data &&
      (!newest || (reading.limits.at || 0) > (newest.limits!.at || 0))
    )
      newest = reading;
  return (
    newest ??
    readings.find((reading) => !reading.limits) ??
    readings.find((reading) => reading.limits?.error) ??
    readings[0]
  );
}

export function serverAccounts(
  store: ServerServices["store"],
  id: string,
  servicesFor?: (id: string) => Pick<ServerServices, "store"> | undefined,
) {
  const selected = servicesFor ? servicesFor(id)?.store : store;
  return selected
    ? (resourceData(selected.state().current)?.accountsSummary.get("current")
        ?.accounts ?? [])
    : [];
}

/** "just now", "5 min ago", "3 h ago", "2 d ago" for an age in seconds. */
export function readingAge(seconds: number) {
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)} min ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} h ago`;
  return `${Math.floor(seconds / 86400)} d ago`;
}

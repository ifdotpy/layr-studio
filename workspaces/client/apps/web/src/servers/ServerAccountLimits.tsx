import { observer } from "mobx-react-lite";
import { useEffect, useState, type ReactNode } from "react";
import { resourceData } from "../store/runtime.ts";
import {
  accountLimits,
  type AccountLimitsData,
} from "../usage/accountUsage.ts";
import AccountStartThrottle from "../components/AccountStartThrottle";
import AccountGroup from "../components/AccountGroup";
import { accountLimitView } from "../components/AccountTiles";
import { AccountCapacityView, type Account } from "../components/Accounts";
import {
  useServerStore,
  ServerServicesProvider,
} from "./ServerServicesContext";
import type { ServerAccount } from "./navigation";
import type { ServerServices } from "./serverServices";

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

/** Requests in this subtree go to the given server, or to the default API. */
function ServerScope({
  services,
  children,
}: {
  services?: ServerServices | undefined;
  children: ReactNode;
}) {
  return services ? (
    <ServerServicesProvider services={services}>
      {children}
    </ServerServicesProvider>
  ) : (
    children
  );
}

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

/** The remaining limit of one provider account, shown once for all servers
 * where it is signed in. Each server reads through its own API and cache. */
export const AccountLimitsAcrossServers = observer(
  function AccountLimitsAcrossServers({
    sources,
    extraDetails,
    facts,
  }: {
    sources: AccountSource[];
    extraDetails?: ReactNode;
    facts?: ReactNode;
  }) {
    const store = useServerStore();
    const [now, setNow] = useState(() => Date.now() / 1000);
    useEffect(() => {
      const timer = setInterval(() => setNow(Date.now() / 1000), 30_000);
      return () => clearInterval(timer);
    }, []);
    const shown = newestReading(
      sources.map((source) => {
        const projection =
          resourceData(
            (source.services?.store ?? store).state().current,
          )?.rateLimitsByAccount.get(source.account.id) ?? null;
        return {
          source,
          projection,
          limits: accountLimits(
            projection?.observation,
            source.account.id,
            source.account.accountId,
          ),
        };
      }),
    );
    const account = shown?.source.account ?? sources[0]?.account;
    if (!account) return null;
    const at = shown?.limits?.data ? shown.limits.at : null;
    const note =
      typeof at === "number"
        ? `Updated ${readingAge(Math.max(0, now - at))}${
            sources.length > 1 ? ` on ${shown!.source.serverLabel}` : ""
          }`
        : undefined;
    return (
      <AccountCapacityView
        card
        account={account}
        view={accountLimitView(
          account,
          shown?.limits ?? null,
          true,
          shown?.projection ?? null,
        )}
        note={note}
        extraDetails={extraDetails}
        facts={facts}
      />
    );
  },
);

/** Turn start limits and the account group are settings of one Studio server,
 * stored on that server's account, so the request goes to that server. */
export function ServerTurnStartLimits({
  source,
  onSaved,
}: {
  source: AccountSource;
  onSaved: (accounts: readonly Account[]) => void;
}) {
  return (
    <ServerScope services={source.services}>
      <AccountStartThrottle
        account={source.account}
        serverLabel={source.serverLabel}
        onSaved={(state) => onSaved(state.accounts)}
      />
      <AccountGroup
        account={source.account}
        serverLabel={source.serverLabel}
        onSaved={(state) => onSaved(state.accounts)}
      />
    </ServerScope>
  );
}

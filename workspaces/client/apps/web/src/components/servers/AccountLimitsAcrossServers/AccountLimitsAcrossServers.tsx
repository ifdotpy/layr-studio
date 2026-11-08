import {
  type AccountSource,
  newestReading,
  readingAge,
} from "../../../servers/ServerAccountLimits.tsx";
import { observer } from "mobx-react-lite";
import { useEffect, useState, type ReactNode } from "react";
import { resourceData } from "../../../store/runtime.ts";
import { accountLimits } from "../../../usage/accountUsage.ts";
import { accountLimitView } from "../../accounts/support/AccountTiles.tsx";
import { AccountCapacityView } from "../../accounts/AccountCapacityView/AccountCapacityView.tsx";
import { useServerStore } from "../ServerServicesProvider/ServerServicesProvider.tsx";

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

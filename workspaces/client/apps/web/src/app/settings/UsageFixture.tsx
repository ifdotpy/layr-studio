import { useEffect, useState } from "react";
import { serializeStateCollections } from "../../generated/observableResources.ts";
import { observableFixture } from "../../components/observableFixture.tsx";
import {
  useServerStore,
  ServerStoreProvider,
} from "../../servers/ServerServicesContext.tsx";
import { resourceData } from "../../store/runtime.ts";
import Usage from "../../components/Usage.tsx";
import type { Account } from "../../components/Accounts.tsx";
import type { DeepReadonly } from "../../deepReadonly.ts";
import type { AccountLimitsData } from "../../usage/accountUsage.ts";
import type { components } from "../../generated/api.ts";
export type UsageAccount = DeepReadonly<
  Pick<Account, "label"> &
    Partial<Pick<Account, "email" | "plan" | "accountId" | "provider">> &
    Partial<
      Pick<
        components["schemas"]["AccountLimitsProjection"],
        "runway" | "reset" | "buckets" | "status" | "agents"
      >
    > & {
      key: string;
      limits: AccountLimitsData | null;
      loading: boolean;
      reload: (force?: boolean) => void | Promise<void>;
    }
>;
import type { Agent } from "../../types.ts";
import { projectedLimits } from "../../usage/accountLimits.fixture.ts";

/** Story/test inputs enter through the generated store, just as live entities do. */
export function UsageFixture({
  agent,
  accounts = [],
  opened = false,
  visible = true,
  onChange,
}: {
  agent: Agent;
  accounts?: UsageAccount[];
  opened?: boolean;
  visible?: boolean;
  stateDir?: string;
  limits?: UsageAccount["limits"];
  reload?: () => unknown;
  onChange?: (opened: boolean) => void;
  accountLabel?: string;
  limitsLoading?: boolean;
}) {
  const parent = useServerStore();
  const [fixture] = useState(() => {
    const models = observableFixture([agent]);
    models.apply([
      ...serializeStateCollections(models.mirror.current),
      {
        collection: "accounts",
        id: "current",
        value: {
          accounts: accounts.map((account) => ({
            id: account.key,
            label: account.label,
            email: account.email,
            provider: account.provider || "codex",
            plan: account.plan,
            accountId: account.accountId,
            status: "ready",
            home: "",
            source: "fixture",
          })),
          archivedAccounts: [],
          defaultAccountKey: agent.accountKey || "default",
          defaultAccountGroups: [],
          supportsDelete: true,
          supportsDisconnect: true,
        },
      },
      ...accounts.map((account) => {
        const base = projectedLimits(account.key, account.limits?.at || 0);
        return {
          collection: "rateLimit" as const,
          id: account.key,
          value: {
            observation: account.limits || base.observation,
            projection: {
              ...base.projection,
              ...(account.runway && { runway: account.runway }),
              ...(account.buckets && { buckets: account.buckets }),
              ...(account.reset && { reset: account.reset }),
              ...(account.status && { status: account.status }),
              ...(account.agents && { agents: account.agents }),
            },
          },
        };
      }),
    ]);
    const store = {
      ...models.store,
      agentDetail: parent.agentDetail,
      sessionCost: parent.sessionCost,
      costs: parent.costs,
    };
    return { store, dispose: models.store.dispose };
  });
  useEffect(() => () => fixture.dispose(), [fixture]);
  return (
    <ServerStoreProvider value={fixture.store}>
      <Usage
        agent={
          resourceData(parent.state().current)?.agents.get(agent.id) || agent
        }
        opened={opened}
        {...(onChange ? { onChange } : {})}
        visible={visible}
      />
    </ServerStoreProvider>
  );
}

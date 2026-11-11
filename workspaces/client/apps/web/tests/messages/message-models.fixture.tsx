import type { ReactNode } from "react";
import {
  createStateMirror,
  type StateEntity,
} from "../../src/generated/observableResources.ts";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createServerStore } from "../../src/store/server.ts";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events.ts";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
import type { Agent } from "../../src/types.ts";
import {
  ConversationProvider,
  type ConversationActions,
} from "../../src/components/conversation/support/navigation.ts";

/** Static state facts with the real room/complaint resource acquisition owner. */
export function messageModels(stateDir: string, agents: readonly Agent[]) {
  const mirror = createStateMirror();
  const runtime = createServerStore(defaultSubscriptions);
  const store: typeof runtime = {
    ...runtime,
    state: () => ({ current: { status: "live", data: mirror.current } }),
  };
  return {
    store,
    apply(entities: readonly StateEntity[]) {
      mirror.apply([
        { collection: "workspace", id: "workspace", value: { stateDir } },
        ...agents.map((agent) => ({
          collection: "agentSummary" as const,
          id: agent.id,
          value: {
            ...agentSummaryFixture(agent.id),
            ...agent,
            isLead: agent.isLead === true,
            inFlight: agent.inFlight === true,
            projectServerId: agent.projectServerId ?? null,
          },
        })),
        ...entities,
      ]);
    },
    Provider({
      commands,
      children,
    }: {
      commands: ConversationActions;
      children?: ReactNode;
    }) {
      return (
        <ServerStoreProvider value={store}>
          <ConversationProvider value={commands}>
            {children}
          </ConversationProvider>
        </ServerStoreProvider>
      );
    },
    close: () => store.dispose(),
  };
}

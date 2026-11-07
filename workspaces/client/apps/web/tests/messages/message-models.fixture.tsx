import type { ReactNode } from "react";
import {
  createStateMirror,
  type StateEntity,
} from "../../src/generated/observableResources";
import { ServerStoreProvider } from "../../src/servers/ServerServicesContext";
import { createServerStore } from "../../src/store";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture";
import type { Agent } from "../../src/types";
import {
  ConversationProvider,
  type ConversationActions,
} from "../../src/components/conversation/navigation";

/** Static state facts with the real room/complaint resource acquisition owner. */
export function messageModels(stateDir: string, agents: readonly Agent[]) {
  const mirror = createStateMirror();
  const runtime = createServerStore(defaultSubscriptions);
  const store: typeof runtime = {
    ...runtime,
    state: () => ({ current: { status: "live", data: mirror.current } }),
  };
  return {
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

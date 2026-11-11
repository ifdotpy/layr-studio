export const fixtureServerStore = createServerStore(defaultSubscriptions);
window.addEventListener("pagehide", () => fixtureServerStore.dispose(), {
  once: true,
});
import { createServerStore } from "../../src/store/server.ts";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events.ts";
import type { ReactNode } from "react";
import {
  ServerStoreProvider,
  useServerStore,
} from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../src/store/runtime.ts";
import { agentId } from "../../src/generated/observableResources.ts";
import type { Agent } from "../../src/types.ts";

export function PublishedAgentFixture({ children }: { children: ReactNode }) {
  return (
    <ServerStoreProvider value={fixtureServerStore}>
      {children}
    </ServerStoreProvider>
  );
}
export function usePublishedAgent(id: string, initial: Agent): Agent {
  const store = useServerStore();
  const summary = resourceData(store.state().current)?.agents.get(id);
  const detail = resourceData(store.agentDetail(agentId(id)).current);
  return { ...initial, ...summary, ...detail };
}
/** This endpoint belongs to the fixture server. The response carries no resource data. */
export async function publishFixtureAgent(agent: Agent): Promise<void> {
  const response = await fetch("/fixture/agent", {
    method: "POST",
    body: JSON.stringify(agent),
  });
  if (!response.ok) throw new Error("Fixture publication failed");
}

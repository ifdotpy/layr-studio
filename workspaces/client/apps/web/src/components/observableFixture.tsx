import { observer } from "mobx-react-lite";
import { useAgentDetail } from "./agents/state.ts";
import { ExecutionSettings } from "./agents/ExecutionSettings.tsx";
import type { ComponentProps } from "react";
import { useEffect, useLayoutEffect, useState, type ReactNode } from "react";
import {
  createStateMirror,
  createAgentDetailMirror,
  createProcessesMirror,
  type AgentId,
} from "../generated/observableResources.ts";

import type { Agent } from "../types.ts";
import type { SidebarAgent } from "./sidebar/viewModel.ts";
import { agentSummaryFixture } from "../servers/agentSummaryFixture.ts";
import { ServerStoreProvider } from "../servers/ServerServicesContext.tsx";
import { createServerStore } from "../store/index.ts";
import { createSubscriptions } from "../sync/subscriptions/module.ts";

/** Keeps controlled browser/story facts in their private generated models. */
export function AgentModelsFixture({
  agents,
  children,
}: {
  agents: readonly Agent[];
  children: ReactNode;
}) {
  const [models] = useState(() => observableFixture(agents));
  useLayoutEffect(() => {
    for (const agent of agents) {
      const detail = models.details.get(agent.id);
      if (detail) detail.apply(agent);
      else models.details.set(agent.id, createAgentDetailMirror(agent));
    }
    models.apply(agentEntities(agents));
  }, [agents, models]);
  useEffect(() => () => models.store.dispose(), [models]);
  return <models.Provider>{children}</models.Provider>;
}

function agentEntities(agents: readonly Agent[]) {
  return agents.map(
    (
      agent,
    ): { collection: "agentSummary"; id: string; value: SidebarAgent } => ({
      collection: "agentSummary",
      id: agent.id,
      value: {
        ...agentSummaryFixture(agent.id),
        ...agent,
        inFlight: agent.inFlight === true,
        isLead: agent.isLead === true,
        projectServerId: agent.projectServerId ?? null,
      },
    }),
  );
}

/** Explicit generated models for isolated stories and renderer assertions. */
export function observableFixture(
  agents: readonly (Agent & Pick<SidebarAgent, "hasBackgroundWork">)[] = [],
) {
  const mirror = createStateMirror();
  const details = new Map(
    agents.map((agent) => [agent.id, createAgentDetailMirror(agent)]),
  );
  mirror.apply(agentEntities(agents));
  const processes = createProcessesMirror({ processes: [], activities: [] });
  const subscriptions = createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: (callback) => {
      callback("live", "open", {});
      return () => {};
    },
  });
  const runtime = createServerStore(subscriptions);
  const store: ReturnType<typeof createServerStore> = {
    ...runtime,
    state: () => ({ current: { status: "live", data: mirror.current } }),
    agentDetail: (id: AgentId) => {
      const detail = details.get(id);
      return {
        current: detail
          ? { status: "live", data: detail.current }
          : { status: "loading" },
      };
    },
    processes: () => ({ current: { status: "live", data: processes.current } }),
    dispose: () => {
      runtime.dispose();
      subscriptions.dispose();
    },
  };
  return {
    store,
    mirror,
    details,
    processes,
    apply(rows: Parameters<typeof mirror.apply>[0]) {
      mirror.apply(rows);
    },
    Provider({ children }: { children?: ReactNode }) {
      return (
        <ServerStoreProvider value={store}>{children}</ServerStoreProvider>
      );
    },
  };
}

export function conversationActionsFixture(
  open: (id: string) => void = () => {},
): import("./conversation/navigation.ts").ConversationActions {
  return {
    open,
    nativeAction: () => {},
    openActivity: () => {},
    read: {
      marking: new Set(),
      pendingReadTargets: new Map(),
      markSidebarUnread: async () => {},
    },
  };
}

/** Standalone settings fixtures use the same explicit server input as their caller. */
export const ExecutionSettingsFixture = observer(
  function ExecutionSettingsFixture({
    agent,
    lead,
    ...props
  }: Omit<ComponentProps<typeof ExecutionSettings>, "input" | "leadInput"> & {
    agent: Agent;
    lead?: Agent | undefined;
  }) {
    const detail = useAgentDetail(agent);
    const leadDetail = useAgentDetail(lead);
    return (
      <ExecutionSettings
        {...props}
        input={{ kind: "agent", summary: agent, detail }}
        leadInput={
          lead
            ? { kind: "agent", summary: lead, detail: leadDetail }
            : undefined
        }
      />
    );
  },
);

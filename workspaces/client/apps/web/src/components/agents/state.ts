import { resourceData } from "../../store/runtime.ts";
import { useMemo } from "react";
import { makeAutoObservable } from "mobx";
import { useServerStore } from "../../servers/ServerServicesContext.tsx";
import { agentId } from "../../generated/observableResources.ts";
import { AgentSource } from "../../generated/api.ts";
import type { SidebarAgent } from "../sidebar/viewModel.ts";
import {
  formatAgentStatus,
  type ReadState,
} from "../../status/operationalStatus.ts";
import {
  orderWorkerHierarchy,
  teamDispatchGroups,
  filterTeamWorkers,
} from "./teamModel.ts";
import {
  chatActivities,
  backgroundActivities,
} from "../chat-status/chatActivity.ts";

type Store = ReturnType<typeof useServerStore>;
function collections(store: Store) {
  const current = store.state().current;
  return resourceData(current);
}
export function useStateCollections() {
  return collections(useServerStore());
}
export function useAgentSummary(id: string) {
  return useStateCollections()?.agents.get(id);
}
/** Only an observed leaf opens detail interest in its owning server. */
export function useAgentDetail(agent: Pick<SidebarAgent, "id"> | undefined) {
  const store = useServerStore();
  if (!agent) return undefined;
  const current = store.agentDetail(agentId(agent.id)).current;
  return resourceData(current);
}
export function useWorkspace() {
  return useStateCollections()?.workspaces.values().next().value;
}

export function useActivityView(agent: SidebarAgent) {
  const store = useServerStore();
  return useMemo(() => {
    const view = {
      get activities() {
        const state = collections(store);
        const processes = store.processes(
          agentId(agent.rootId || agent.id),
        ).current;
        if (
          !state ||
          (processes.status !== "live" && processes.status !== "stale")
        )
          return [];
        const members = [...state.agents.values()].filter(
          (a) => a.id === agent.id || a.rootId === (agent.rootId || agent.id),
        );
        return (
          chatActivities(members, processes.data.activities).get(agent.id) || []
        );
      },
      get background() {
        return backgroundActivities(this.activities);
      },
      get commands() {
        return this.activities.filter(
          (activity) =>
            activity.agentId === agent.id &&
            activity.kind === "task" &&
            activity.commandTask &&
            (activity.background || !agent.inFlight),
        );
      },
      get monitors() {
        return this.activities.filter(
          (activity) =>
            activity.agentId === agent.id && activity.kind === "monitor",
        );
      },
    };
    return makeAutoObservable(view);
  }, [store, agent]);
}

export function useConversationStartView(agent: SidebarAgent) {
  const store = useServerStore();
  return useMemo(() => {
    const view = {
      get name() {
        return (
          [...(collections(store)?.projects.values() || [])].find(
            (project) => project.path === agent.projectPath,
          )?.name ||
          agent.projectPath?.split("/").filter(Boolean).at(-1) ||
          "Your project"
        );
      },
      get recent() {
        return [...(collections(store)?.agents.values() || [])]
          .filter(
            (chat) =>
              chat.id !== agent.id &&
              chat.isLead &&
              !chat.archived &&
              !chat.empty &&
              chat.projectPath === agent.projectPath,
          )
          .sort(
            (a, b) =>
              (b.updated || b.created || 0) - (a.updated || a.created || 0),
          )
          .slice(0, 5);
      },
    };
    return makeAutoObservable(view);
  }, [store, agent]);
}

export function useTeamView(
  leadId: string | undefined,
  query = "",
  filter = "all",
  pendingReads?: ReadonlyMap<string, ReadState>,
) {
  const store = useServerStore();
  return useMemo(
    () => createTeamView(store, leadId, query, filter, pendingReads),
    [store, leadId, query, filter, pendingReads],
  );
}
export function createTeamView(
  store: Store,
  leadId: string | undefined,
  query = "",
  filter = "all",
  pendingReads?: ReadonlyMap<string, ReadState>,
) {
  const normalizedQuery = query.trim().toLowerCase();
  const view = {
    query: normalizedQuery,
    indicator: (agent: SidebarAgent) =>
      formatAgentStatus(agent, pendingReads?.get(agent.id)),
    pendingRead: (agent: SidebarAgent) => pendingReads?.get(agent.id),
    get members(): SidebarAgent[] {
      return leadId
        ? [...(collections(store)?.agents.values() || [])].filter(
            (agent) =>
              agent.source === AgentSource.managed &&
              (agent.id === leadId || agent.rootId === leadId),
          )
        : [];
    },
    get lead() {
      return this.members.find((agent) => agent.id === leadId);
    },
    get workers(): SidebarAgent[] {
      return orderWorkerHierarchy(
        this.members.filter((agent) => !agent.isLead),
      );
    },
    get indicators() {
      return new Map(
        this.workers.map((agent) => [agent.id, this.indicator(agent)]),
      );
    },
    get groups() {
      return teamDispatchGroups(this.workers, this.indicators);
    },
    get byId() {
      return new Map(this.members.map((agent) => [agent.id, agent]));
    },
    get smallTeam() {
      return this.workers.length <= 3;
    },
    get showFilters() {
      return this.workers.length > 3 || !!query || filter !== "all";
    },
    get shown(): SidebarAgent[] {
      return normalizedQuery
        ? this.workers.filter((agent) => {
            const current = store.agentDetail(agentId(agent.id)).current;
            const overview = resourceData(current)?.overview;
            return `${agent.name} ${agent.status} ${agent.role || ""} ${overview?.task || ""} ${overview?.result || ""}`
              .toLowerCase()
              .includes(normalizedQuery);
          })
        : this.filter(filter);
    },
    get dispatch() {
      return teamDispatchGroups(this.shown, this.indicators);
    },
    get counts() {
      return [
        { state: "all", label: "All", count: this.workers.length },
        ...this.groups.groups.map((group) => ({
          state: group.kind,
          label: group.name,
          count: group.workers.length,
        })),
        {
          state: "completed",
          label: "Finished",
          count: this.groups.completed.length,
        },
      ];
    },
    filter(value: string): SidebarAgent[] {
      return filterTeamWorkers(this.workers, this.indicators, value);
    },
  };
  return makeAutoObservable(view);
}

/** Membership depends on generated models, never a parallel App snapshot index. */
export function useAgentCatalog() {
  const store = useServerStore();
  return agentCatalog(store);
}
const catalogs = new WeakMap<Store, ReturnType<typeof createAgentCatalog>>();
export function agentCatalog(store: Store) {
  let view = catalogs.get(store);
  if (!view) {
    view = createAgentCatalog(store);
    catalogs.set(store, view);
  }
  return view;
}
export function createAgentCatalog(store: Store) {
  const view = {
    get agents(): SidebarAgent[] {
      return [...(collections(store)?.agents.values() || [])];
    },
    get leads(): SidebarAgent[] {
      return this.agents.filter(
        (agent) =>
          agent.source === AgentSource.managed &&
          agent.isLead &&
          !agent.sharedRoomId,
      );
    },
    get(id: string) {
      return collections(store)?.agents.get(id);
    },
    errors(account: string, selected?: string, root?: string) {
      return this.agents
        .filter(
          (agent) =>
            (agent.id === selected || agent.rootId === root) &&
            (agent.accountKey || "default") === account,
        )
        .flatMap((agent) => {
          const detail = store.agentDetail(agentId(agent.id)).current;
          const data = resourceData(detail);
          if (!data) return [];
          const native = data.nativeStatus;
          return [
            data.error,
            native && typeof native === "object" && "error" in native
              ? native.error
              : undefined,
          ].filter((value): value is string => typeof value === "string");
        });
    },
  };
  return makeAutoObservable(view);
}

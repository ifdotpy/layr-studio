import { useMemo } from "react";
import { computed, compareShallow, makeAutoObservable } from "mobx";
import { useServerStore } from "../../servers/ServerServicesContext";
import type { RequestModel } from "./requestActions";

export type RequestScope = {
  agentId?: string | null | undefined;
  familyId?: string | null | undefined;
  includeTeam?: boolean;
  all?: boolean;
  turnId?: string | null | undefined;
  pendingOnly?: boolean;
};
export function useRequestsView(scope: RequestScope) {
  const store = useServerStore();
  const { agentId, familyId, includeTeam, all, turnId, pendingOnly } = scope;
  return useMemo(
    () =>
      makeAutoObservable(
        {
          get current() {
            return store.requests().current;
          },
          get allRequests(): readonly RequestModel[] {
            const current = this.current;
            return current.status === "live" || current.status === "stale"
              ? [...current.data.requests.values()]
              : [];
          },
          get agents() {
            const current = store.state().current;
            return current.status === "live" || current.status === "stale"
              ? current.data.agents
              : undefined;
          },
          get owner() {
            return agentId ? this.agents?.get(agentId) : undefined;
          },
          get rootId() {
            return familyId || this.owner?.rootId || agentId;
          },
          get familyRequests() {
            return this.allRequests.filter((request) => {
              const owner = this.agents?.get(request.agent);
              return (owner?.rootId || request.agent) === this.rootId;
            });
          },
          get requests() {
            return this.allRequests.filter((request) => {
              if (pendingOnly && request.state.kind !== "pending") return false;
              if (turnId && request.turn && request.turn !== turnId)
                return false;
              if (all || !request.agent) return true;
              if (request.agent === agentId) return true;
              if (!includeTeam) return false;
              const owner = this.agents?.get(request.agent);
              return owner?.rootId === this.rootId;
            });
          },
        },
        {
          familyRequests: computed({ equals: compareShallow }),
          requests: computed({ equals: compareShallow }),
        },
      ),
    [store, agentId, familyId, includeTeam, all, turnId, pendingOnly],
  );
}

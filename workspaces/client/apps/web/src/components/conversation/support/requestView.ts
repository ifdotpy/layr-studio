import { computed, compareShallow, makeAutoObservable } from "mobx";
import type { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { RequestModel } from "../questions/RequestArea/RequestArea.tsx";

export type RequestScope = {
  agentId?: string | null | undefined;
  familyId?: string | null | undefined;
  includeTeam?: boolean;
  all?: boolean;
  turnId?: string | null | undefined;
  pendingOnly?: boolean;
};
export function createRequestsView(
  store: ReturnType<typeof useServerStore>,
  scope: RequestScope | (() => RequestScope),
) {
  return makeAutoObservable(
    {
      get scope() {
        return typeof scope === "function" ? scope() : scope;
      },
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
        return this.scope.agentId
          ? this.agents?.get(this.scope.agentId)
          : undefined;
      },
      get rootId() {
        return this.scope.familyId || this.owner?.rootId || this.scope.agentId;
      },
      get familyRequests(): readonly RequestModel[] {
        return this.allRequests.filter((request) => {
          const owner = this.agents?.get(request.agent);
          return (owner?.rootId || request.agent) === this.rootId;
        });
      },
      get requests(): readonly RequestModel[] {
        return this.select(this.scope);
      },
      select({
        agentId,
        familyId,
        includeTeam,
        all,
        turnId,
        pendingOnly,
      }: RequestScope): readonly RequestModel[] {
        const rootId =
          familyId ||
          (agentId ? this.agents?.get(agentId)?.rootId : undefined) ||
          agentId;
        return this.allRequests.filter((request) => {
          if (pendingOnly && request.state.kind !== "pending") return false;
          if (turnId && request.turn && request.turn !== turnId) return false;
          if (all || !request.agent) return true;
          if (request.agent === agentId) return true;
          if (!includeTeam) return false;
          const owner = this.agents?.get(request.agent);
          return owner?.rootId === rootId;
        });
      },
    },
    {
      familyRequests: computed({ equals: compareShallow }),
      requests: computed({ equals: compareShallow }),
    },
    { autoBind: true },
  );
}

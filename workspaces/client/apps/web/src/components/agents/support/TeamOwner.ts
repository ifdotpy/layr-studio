import { makeAutoObservable, reaction, autorun } from "mobx";
import type { ServerAreas } from "../../../app/contracts.ts";
import { AgentStatus } from "../../../generated/api.ts";
import { agentId } from "../../../generated/observableResources.ts";
import { resourceData } from "../../../store/runtime.ts";
import { createTeamView } from "./state.ts";

export class TeamOwner {
  workerQuery = "";
  workerFilter = "all";
  override: boolean | null = null;
  readonly stops;
  readonly areas;
  constructor(areas: ServerAreas) {
    this.areas = areas;
    makeAutoObservable(this, { areas: false }, { autoBind: true });
    this.stops = [
      reaction(
        () => areas.navigation.selectedId,
        () => {
          this.override = null;
        },
      ),
      reaction(
        () => areas.navigation.selectedLead?.id,
        () => {
          this.workerQuery = "";
          this.workerFilter = "all";
        },
      ),
      autorun(() => {
        if (areas.ui.isTeamVisible && this.view.members.length)
          areas.scope.services.tokenRates.observeTokenRates();
      }),
    ];
  }
  get view() {
    const a = this.areas;
    return createTeamView(
      a.scope.store,
      a.scope.screen.active ? a.navigation.selectedLead?.id : undefined,
      this.workerQuery,
      this.workerFilter,
      a.read.pendingReadTargets,
    );
  }
  get opened() {
    return this.areas.navigation.selectedId;
  }
  get completedOpen() {
    const selected = this.areas.navigation.selectedAgent;
    return (
      this.override ??
      (!!selected &&
        !selected.isLead &&
        selected.status === AgentStatus.completed)
    );
  }
  setWorkerQuery(query: string) {
    this.workerQuery = query;
  }
  setWorkerFilter(filter: string) {
    this.workerFilter = filter;
  }
  setCompletedOpen(value: boolean) {
    this.override = value;
  }
  setTeamOpen(value: boolean) {
    this.areas.ui.setTeamOpen(value);
  }
  setWideTeamOpen(value: boolean) {
    this.areas.ui.setTeamPreferred(value);
  }
  open(id: string) {
    this.areas.open(id);
  }
  previewResult(target: Parameters<ServerAreas["ui"]["showPreview"]>[0]) {
    this.areas.ui.showPreview(target);
  }
  remove(id: string) {
    this.areas.ui.requestDelete({ id }, false, true);
  }
  hasTeam() {
    return this.view.workers.length > 0 || this.areas.selected.hasRetainedTeam;
  }
  retainedWorktrees() {
    const id = this.areas.navigation.selectedLead?.id;
    return (
      id
        ? (resourceData(
            this.areas.scope.store.retainedWorktrees(agentId(id)).current,
          ) ?? [])
        : []
    ).filter(
      (item) => !this.view.workers.some((worker) => worker.id === item.id),
    );
  }
  dispose() {
    this.stops.forEach((stop) => stop());
  }
}

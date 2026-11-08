import type { ReadState } from "../../../status/operationalStatus.ts";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { agentId } from "../../../generated/observableResources.ts";
import { resourceData } from "../../../store/runtime.ts";
import { createArea } from "../../../app/area.ts";
import type { PreviewTarget } from "../../conversation/FilePreview/FilePreview.tsx";
import { useTeamView } from "./state.ts";

export interface TeamControls {
  opened: string | null;
  workerQuery: string;
  setWorkerQuery: (query: string) => void;
  workerFilter: string;
  setWorkerFilter: (filter: string) => void;
  completedOpen: boolean;
  setCompletedOpen: (open: boolean) => void;
  setTeamOpen: (open: boolean) => void;
  setWideTeamOpen: (open: boolean) => void;
}

export type TeamActions = {
  open: (id: string) => void;
  previewResult: (target: PreviewTarget) => void;
  remove: (id: string) => void;
};

export const [TeamProvider, useTeam] = createArea<
  TeamControls & TeamActions & ReturnType<typeof useTeamArea>
>("Team");

/** The team root resolves resources; observed leaves read their native models. */
export function useTeamArea(
  leadId: string | undefined,
  query: string,
  filter: string,
  pendingReads?: ReadonlyMap<string, ReadState>,
  hasRetained = false,
) {
  const view = useTeamView(leadId, query, filter, pendingReads);
  const store = useServerStore();
  const retained = leadId
    ? store.retainedWorktrees(agentId(leadId))
    : undefined;
  const retainedWorktrees = () =>
    (retained ? (resourceData(retained.current) ?? []) : []).filter(
      (item) => !view.workers.some((worker) => worker.id === item.id),
    );
  return {
    view,
    retainedWorktrees,
    hasTeam: () => view.workers.length > 0 || hasRetained,
  };
}

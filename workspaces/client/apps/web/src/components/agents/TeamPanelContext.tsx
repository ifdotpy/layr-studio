import type { ReadState } from "../../status/operationalStatus.ts";
import { observer } from "mobx-react-lite";
import { useRetainedWorktrees } from "./RetainedWorktrees.tsx";
import type { ReactNode } from "react";
import { createArea } from "../area.ts";
import type { PreviewTarget } from "../FilePreview.tsx";
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

/** One team area owns both generated membership and the remaining legacy cleanup feed. */
export function useTeamArea(
  leadId: string | undefined,
  query: string,
  filter: string,
  pendingReads?: ReadonlyMap<string, ReadState>,
) {
  const view = useTeamView(leadId, query, filter, pendingReads);
  const retainedWorktrees = useRetainedWorktrees(leadId, view.workers);
  return {
    view,
    retainedWorktrees,
    hasTeam: () => view.workers.length > 0 || retainedWorktrees.length > 0,
  };
}

export const TeamPresence = observer(function TeamPresence({
  children,
}: {
  children: ReactNode;
}) {
  return useTeam().hasTeam() ? children : null;
});
export const TeamCount = observer(function TeamCount({
  className,
}: {
  className?: string;
}) {
  const counts = useTeam().view.lead?.teamCounts;
  if (!counts || (className && counts.total === 0)) return null;
  const text = `${counts.working}/${counts.total}`;
  return className ? <span className={className}>{text}</span> : text;
});

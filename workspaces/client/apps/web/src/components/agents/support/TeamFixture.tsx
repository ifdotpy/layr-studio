import type { ReactNode } from "react";
import TeamPanel from "../TeamPanelView/TeamPanelView.tsx";
import {
  TeamProvider,
  useTeamArea,
  type TeamControls,
  type TeamActions,
} from "./TeamPanelContext.tsx";
import type { ReadState } from "../../../status/operationalStatus.ts";
const noop = () => {};
const defaults: TeamControls & TeamActions = {
  opened: null,
  workerQuery: "",
  workerFilter: "all",
  completedOpen: false,
  setWorkerQuery: noop,
  setWorkerFilter: noop,
  setCompletedOpen: noop,
  setTeamOpen: noop,
  setWideTeamOpen: noop,
  open: noop,
  previewResult: noop,
  remove: noop,
};
export default function TeamFixture({
  leadId,
  actions,
  pendingReads,
  hasRetained = false,
  children,
  ...controls
}: Partial<TeamControls> & {
  hasRetained?: boolean;
  leadId?: string | undefined;
  actions?: TeamActions;
  pendingReads?: ReadonlyMap<string, ReadState>;
  children?: ReactNode;
}) {
  const area = useTeamArea(
    leadId,
    controls.workerQuery || "",
    controls.workerFilter || "all",
    pendingReads,
    hasRetained,
  );
  return (
    <TeamProvider value={{ ...defaults, ...controls, ...area, ...actions }}>
      {children ?? <TeamPanel />}
    </TeamProvider>
  );
}

import { observer } from "mobx-react-lite";
import type { SidebarChatRow } from "../../generated/observableResources.ts";
import type { components } from "../../generated/api";

type Progress = Extract<
  components["schemas"]["AgentEntityStartWaitDto"],
  { reason: "workspace_copy" }
>["progress"];

const labels: Record<Progress["phase"], string> = {
  sizing: "Checking files",
  staging: "Copying files",
  importing_commits: "Importing history",
  materializing: "Preparing files",
  restoring_attributes: "Restoring file attributes",
  warming_caches: "Preparing caches",
  transferring: "Transferring files",
  creating_line: "Creating workspace line",
  archiving: "Building archive",
  uploading: "Uploading archive",
  importing_project: "Importing project",
  completed: "Copy complete",
  failed: "Copy failed",
};

export default observer(function LayrCopyProgress(
  props: { progress: Progress } | { row: Pick<SidebarChatRow, "copyProgress"> },
) {
  const progress = "row" in props ? props.row.copyProgress : props.progress;
  if (!progress) return null;
  const label = labels[progress.phase];
  const percent =
    progress.total == null
      ? undefined
      : progress.total === 0
        ? 100
        : Math.min(100, (100 * progress.done) / progress.total);
  return (
    <span className="layr-copy-progress">
      <span>
        {label}
        {progress.done > 0 ? ` · ${progress.done} ${progress.unit}` : ""}
      </span>
      <progress aria-label={label} max={100} value={percent} />
    </span>
  );
});

import { useTeam } from "../TeamPanelContext.tsx";
import { observer } from "mobx-react-lite";
import { useAgentDetail } from "../state.ts";
import { workerActivity } from "../workerPresentation.ts";
import type { WorkerProps } from "../WorkerCard.tsx";

const WorkerTask = observer(function WorkerTask({ agent }: WorkerProps) {
  const detail = useAgentDetail(agent);
  return detail?.overview?.task ? (
    <span className="worker-task" title={detail.overview.task}>
      {detail.overview.task}
    </span>
  ) : null;
});

export const WorkerActivity = observer(function WorkerActivity({
  agent,
}: WorkerProps) {
  const detail = useAgentDetail(agent);
  const {
    ActivityIcon,
    error,
    diskHeld,
    stopReason,
    statusDetail,
    activityText,
    activity,
  } = workerActivity(agent, detail, useTeam().view.indicator(agent));
  return (
    <span className="worker-live-line">
      <ActivityIcon
        className="worker-activity-icon"
        aria-label={error && !diskHeld ? "Error reason" : activity.label}
        role="img"
      />
      <span
        className={
          error && !diskHeld && !stopReason
            ? "worker-activity worker-error"
            : "worker-activity"
        }
        title={error || statusDetail}
      >
        {activityText}
      </span>
      <WorkerTask agent={agent} />
    </span>
  );
});

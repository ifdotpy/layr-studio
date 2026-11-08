import { observer } from "mobx-react-lite";
import { useTeam } from "../support/TeamPanelContext.tsx";
import { TeamStateIcon } from "../TeamStateIcon/TeamStateIcon.tsx";
import { Users } from "lucide-react";
import type { ChatIndicatorKind } from "../../../status/operationalStatus.ts";
import { teamFilterValue } from "../support/teamModel.ts";

export const TeamSummary = observer(function TeamSummary() {
  const view = useTeam();
  const { workerFilter: selected, setWorkerFilter, setCompletedOpen } = view;
  const counts = view.view.counts;
  const onFilter = (value: string) => {
    setWorkerFilter(value);
    if (value === "completed") setCompletedOpen(true);
  };
  return (
    <div className="team-overview" aria-label="Team status summary">
      <div
        className="team-summary-filters"
        role="group"
        aria-label="Filter subagents"
      >
        {counts
          .filter((row) => row.count > 0 || row.state === "all")
          .map((row) => {
            const value = teamFilterValue(row.state);
            return (
              <button
                key={row.state}
                type="button"
                data-team-count={row.state}
                aria-label={row.state === "working" ? "Active" : row.label}
                aria-pressed={selected === value}
                title={`Show ${row.label.toLowerCase()} agents`}
                onClick={() => onFilter(selected === value ? "all" : value)}
              >
                {row.state === "all" ? (
                  <Users className="team-state-icon" aria-hidden="true" />
                ) : (
                  <TeamStateIcon
                    kind={
                      row.state === "completed"
                        ? "unread"
                        : (row.state as ChatIndicatorKind)
                    }
                  />
                )}
                <span>{row.label}</span>
                <strong>{row.count}</strong>
              </button>
            );
          })}
      </div>
    </div>
  );
});

export {
  teamFilterValue,
  filterTeamWorkers,
  workerDepth,
  teamDispatchGroups,
  orderWorkerHierarchy,
} from "../support/teamModel.ts";

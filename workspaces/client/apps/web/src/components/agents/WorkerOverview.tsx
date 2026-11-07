import { useAccounts, type Account } from "../Accounts.tsx";
import { useWorkspace } from "./state.ts";
import { createContext, type ReactNode } from "react";
import { useWorkerModels, type WorkerModelInfo } from "./WorkerModelPicker";

export const TeamModelCatalog = createContext<WorkerModelInfo[]>([]);

export const TeamAccountCatalog = createContext<Account[]>([]);
export const TeamModels = observer(function TeamModels({
  children,
}: {
  children: ReactNode;
}) {
  const { view } = useTeam();
  const catalog = useWorkerModels(
    view.lead?.accountKey || "default",
    true,
    true,
  );
  const accounts = useAccounts(useWorkspace()?.stateDir ?? undefined);
  return (
    <TeamModelCatalog.Provider value={catalog.models}>
      <TeamAccountCatalog.Provider value={accounts.data.accounts}>
        {children}
      </TeamAccountCatalog.Provider>
    </TeamModelCatalog.Provider>
  );
});

import { observer } from "mobx-react-lite";
import { useTeam } from "./TeamPanelContext.tsx";
import { TeamStateIcon } from "./TeamFacts";
import { Users } from "lucide-react";
import type { ChatIndicatorKind } from "../../status/operationalStatus.ts";
import { teamFilterValue } from "./teamModel.ts";
export {
  teamFilterValue,
  filterTeamWorkers,
  workerDepth,
  teamDispatchGroups,
  orderWorkerHierarchy,
} from "./teamModel.ts";

export function TeamGroupLabel({
  kind,
  label,
  count,
}: {
  kind: ChatIndicatorKind | "completed";
  label: string;
  count: number;
}) {
  return (
    <span className="team-group-label">
      <TeamStateIcon kind={kind === "completed" ? "unread" : kind} />
      <span>{label}</span>
      <span className="team-group-count">{count}</span>
    </span>
  );
}
export const TeamSummary = observer(function TeamSummary() {
  const {
    view,
    workerFilter: selected,
    setWorkerFilter,
    setCompletedOpen,
  } = useTeam();
  const counts = view.counts;
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

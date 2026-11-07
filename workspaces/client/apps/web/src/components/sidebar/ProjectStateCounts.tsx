import { useMemo } from "react";
import { createProjectStatusView, type SidebarChatView } from "./viewModel.ts";
import { observer } from "mobx-react-lite";
import { aggregateStates, projectStateLabel } from "./fleet";

export default observer(function ProjectStateCounts({
  rows,
  archived,
}: {
  rows: readonly SidebarChatView[];
  archived: boolean;
}) {
  const view = useMemo(
    () => createProjectStatusView(rows, archived),
    [rows, archived],
  );
  const counts = view.counts;
  const label = projectStateLabel(counts);
  if (!label) return null;
  return (
    <span
      className="project-state-counts"
      role="img"
      aria-label={label}
      title={label}
    >
      {aggregateStates
        .filter((kind) => counts[kind])
        .map((kind) => (
          <span className="project-state-count" key={kind} aria-hidden="true">
            <span className="studio-state-dot" data-studio-state={kind} />
            <span>{counts[kind]}</span>
          </span>
        ))}
    </span>
  );
});

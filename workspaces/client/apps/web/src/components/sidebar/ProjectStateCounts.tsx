import { observer } from "mobx-react-lite";
import {
  aggregateStates,
  projectStateLabel,
  type projectStateCounts,
} from "./fleet";

export default observer(function ProjectStateCounts({
  view,
}: {
  view: { readonly counts: ReturnType<typeof projectStateCounts> };
}) {
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

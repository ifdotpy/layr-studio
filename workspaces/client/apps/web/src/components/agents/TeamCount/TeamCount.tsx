import { useTeam } from "../support/TeamPanelContext.tsx";
import { observer } from "mobx-react-lite";

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

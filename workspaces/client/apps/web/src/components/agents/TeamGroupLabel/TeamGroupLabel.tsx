import { TeamStateIcon } from "../TeamStateIcon/TeamStateIcon.tsx";
import type { ChatIndicatorKind } from "../../../status/operationalStatus.ts";

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

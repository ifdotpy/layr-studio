import { workspaceBadgeInfo } from "../support/WorkspaceBadge.tsx";
import type { Agent } from "../../../../types.ts";

export default function WorkspaceBadge({
  agent,
  passive = false,
}: {
  agent?: Agent | undefined;
  passive?: boolean | undefined;
}) {
  const info = workspaceBadgeInfo(agent);
  if (!info) return null;
  return (
    <span
      className="workspace-mode-badge"
      title={info.title}
      aria-label={info.title}
      tabIndex={passive ? undefined : 0}
    >
      {info.label}
    </span>
  );
}

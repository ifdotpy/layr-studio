import {
  CheckCheck,
  Circle,
  CircleAlert,
  CircleHelp,
  LoaderCircle,
  Pause,
} from "lucide-react";
import type { ChatIndicatorKind } from "../../../status/operationalStatus.ts";

export function TeamStateIcon({ kind }: { kind: ChatIndicatorKind }) {
  if (kind === "loading") return null;
  const Icon = stateIcons[kind];
  return (
    <Icon
      className={`team-state-icon team-state-icon-${kind}`}
      aria-hidden="true"
    />
  );
}

const stateIcons = {
  answer: CircleHelp,
  error: CircleAlert,
  working: LoaderCircle,
  unread: CheckCheck,
  paused: Pause,
  none: Circle,
};

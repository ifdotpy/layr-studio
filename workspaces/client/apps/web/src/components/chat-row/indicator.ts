import type { SidebarChatView, SidebarAgent } from "../sidebar/viewModel.ts";
import { formatOperationalStatus } from "../../status/operationalStatus.ts";

export function displayedIndicator(
  row: SidebarChatView,
  pendingRead?: SidebarAgent["markRead"],
) {
  const target = pendingRead ? row.agent.markRead : null;
  if (
    pendingRead &&
    target &&
    pendingRead.threadId === target.threadId &&
    pendingRead.turnId === target.turnId &&
    pendingRead.revision === target.revision &&
    row.indicator.kind === "unread"
  ) {
    return formatOperationalStatus({ reason: "read_result" });
  }
  return row.indicator;
}

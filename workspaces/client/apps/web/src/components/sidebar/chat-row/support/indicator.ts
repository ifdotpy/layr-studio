import type { SidebarChatView, SidebarAgent } from "../../support/viewModel.ts";
import { formatAgentStatus } from "../../../../status/operationalStatus.ts";

export function displayedIndicator(
  row: SidebarChatView,
  pendingRead?: SidebarAgent["markRead"],
) {
  return pendingRead
    ? formatAgentStatus(row.agent, pendingRead)
    : row.indicator;
}

import { ChatOperationalKind } from "../../generated/api.ts";
import type { SidebarChatRow } from "../../generated/observableResources.ts";

export type IndicatorRow = Pick<SidebarChatRow, "indicator" | "provider"> &
  Partial<Pick<SidebarChatRow, "model" | "markRead">>;

export function displayedIndicator(
  row: IndicatorRow,
  pendingRead?: SidebarChatRow["markRead"],
) {
  const target = row.markRead;
  if (
    pendingRead &&
    target &&
    pendingRead.threadId === target.threadId &&
    pendingRead.turnId === target.turnId &&
    pendingRead.revision === target.revision &&
    row.indicator.kind === ChatOperationalKind.unread
  ) {
    return { kind: ChatOperationalKind.none, label: "Read" };
  }
  return row.indicator;
}

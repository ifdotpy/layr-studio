import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Mail } from "lucide-react";
import type { SidebarChatView, SidebarAgent } from "../sidebar/viewModel.ts";
import { MarkUnreadEligibility } from "../../generated/api.ts";
import type { ChatRowActions } from "./actions.ts";

export const UnreadAction = observer(function UnreadAction({
  row,
  disabled,
  markingRead,
  actions,
}: {
  row: SidebarChatView;
  disabled: boolean;
  markingRead: boolean;
  actions: ChatRowActions;
}) {
  const unreadHelp = markingRead
    ? "The read status is changing."
    : markUnreadHelp(row.agent.markUnread);
  return (
    <Menu.Item
      leftSection={<Mail size={14} />}
      aria-label="Mark unread"
      disabled={
        disabled ||
        row.agent.markUnread !== MarkUnreadEligibility.allowed ||
        markingRead
      }
      onClick={() => actions.unread()}
    >
      Mark unread
      {unreadHelp && <small className="menu-action-help">{unreadHelp}</small>}
    </Menu.Item>
  );
});

function markUnreadHelp(
  eligibility: SidebarAgent["markUnread"],
): string | null {
  switch (eligibility) {
    case MarkUnreadEligibility.allowed:
      return null;
    case MarkUnreadEligibility.unsupported:
      return "Read status is unavailable for this chat.";
    case MarkUnreadEligibility.no_completed_result:
      return "Available after an answer.";
    case MarkUnreadEligibility.already_unread:
      return "This answer is already unread.";
  }
  return eligibility satisfies never;
}

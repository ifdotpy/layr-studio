import { MarkUnreadEligibility } from "../generated/api.ts";
import { useConversationActions } from "./conversation/navigation.ts";

import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Mail } from "lucide-react";
import { useAgentSummary } from "./agents/state.ts";

export const MarkChatUnreadItem = observer(function MarkChatUnreadItem({
  agentId,
}: {
  agentId: string;
}) {
  const agent = useAgentSummary(agentId);
  const actions = useConversationActions().read;
  return (
    <Menu.Item
      id="mark-unread"
      aria-label="Mark chat unread"
      leftSection={<Mail size={14} />}
      disabled={
        agent?.markUnread !== MarkUnreadEligibility["allowed"] ||
        !!actions.marking.has(agentId)
      }
      onClick={() => {
        if (agent?.markRead)
          void actions.markSidebarUnread(agent.id, agent.markRead);
      }}
    >
      Mark as unread
    </Menu.Item>
  );
});

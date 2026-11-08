import { MarkUnreadEligibility } from "../generated/api.ts";
import { useConversationActions } from "./conversation/navigation.ts";

import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Mail } from "lucide-react";
import type { AgentModel } from "../app/contracts.ts";

export const MarkChatUnreadItem = observer(function MarkChatUnreadItem({
  agent,
}: {
  agent: AgentModel;
}) {
  const actions = useConversationActions().read;
  return (
    <Menu.Item
      id="mark-unread"
      aria-label="Mark chat unread"
      leftSection={<Mail size={14} />}
      disabled={
        agent?.markUnread !== MarkUnreadEligibility["allowed"] ||
        !!actions.marking.has(agent.id)
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

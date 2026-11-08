import SessionActivity from "../SessionActivity/SessionActivity.tsx";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../store/runtime.ts";
import { useConversationActions } from "../../conversation/support/navigation.ts";
import { observer } from "mobx-react-lite";
import { useActivityView } from "../support/state.ts";
import type { SidebarAgent } from "../../sidebar/support/viewModel.ts";

export const LiveSessionActivity = observer(function LiveSessionActivity({
  agentId,
  compact,
  onProcess,
}: {
  agentId: string;
  compact: boolean;
  onProcess?: ((id: string) => void) | undefined;
}) {
  const store = useServerStore();
  const agent = resourceData(store.state().current)?.agents.get(agentId);
  return agent ? (
    <AgentSessionActivity
      agent={agent}
      compact={compact}
      onProcess={onProcess}
    />
  ) : null;
});

const AgentSessionActivity = observer(function AgentSessionActivity({
  agent,
  compact,
  onProcess,
}: {
  agent: SidebarAgent;
  compact: boolean;
  onProcess?: ((id: string) => void) | undefined;
}) {
  const view = useActivityView(agent);
  const actions = useConversationActions();
  return (
    <SessionActivity
      activities={view.background}
      onOpen={(activity) =>
        activity.kind === "agent"
          ? actions.open(activity.agentId)
          : onProcess?.(activity.id)
      }
      compact={compact}
    />
  );
});

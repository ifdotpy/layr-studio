import { resourceData } from "../../store/runtime.ts";
import { useServerStore } from "../../servers/ServerServicesContext.tsx";
import { agentId as resourceAgentId } from "../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Minimize2, ShieldCheck } from "lucide-react";
import { menuActions, nativeActionDisabled } from "../../nativeCommands.ts";
import type { SidebarAgent } from "../sidebar/viewModel.ts";
import { useConversationActions } from "./navigation.ts";

export const NativeModelActions = observer(function NativeModelActions({
  agent,
}: {
  agent: SidebarAgent;
}) {
  const actions = useConversationActions();
  const store = useServerStore();
  const disabled = nativeActionDisabled(
    agent,
    resourceData(store.agentDetail(resourceAgentId(agent.id)).current),
  );
  return (
    <>
      {(
        [
          ["compact", "Compact", Minimize2],
          ["review", "Review", ShieldCheck],
        ] as const
      )
        .filter(([action]) =>
          menuActions(agent.provider || undefined).includes(action),
        )
        .map(([action, label, Icon]) => (
          <Menu.Item
            key={action}
            aria-label={label}
            data-action={action}
            leftSection={<Icon size={14} />}
            disabled={disabled}
            onClick={() => actions.nativeAction(agent, action)}
          >
            {label}
            {disabled && (
              <small className="menu-action-help">
                {!agent.threadId
                  ? "Available after the chat starts."
                  : "Wait until the chat is ready."}
              </small>
            )}
          </Menu.Item>
        ))}
    </>
  );
});

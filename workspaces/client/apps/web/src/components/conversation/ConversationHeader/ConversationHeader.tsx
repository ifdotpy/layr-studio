import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
import { observer } from "mobx-react-lite";
import { ActionIcon, Menu } from "@mantine/core";
import { MoreHorizontal } from "lucide-react";
import { SidebarToggle } from "../../shell/SidebarToggle/SidebarToggle.tsx";
import { HeaderButtons } from "../../shell/HeaderButtons/HeaderButtons.tsx";
import { LiveSessionActivity } from "../../agents/LiveSessionActivity/LiveSessionActivity.tsx";
import { SelectedTitle } from "../SelectedTitle/SelectedTitle.tsx";
import { ChatActionsMenu } from "../ChatActionsMenu/ChatActionsMenu.tsx";
import { useConversationActions } from "../support/navigation.ts";

export const ConversationHeader = observer(function ConversationHeader() {
  const { navigation, layout } = useConversationActions();
  reportPromptComposerRender("conversation-header");
  return (
    <header
      className="workspace-header simple-workspace-header"
      data-compact-landscape={layout.isCompactHeader || undefined}
    >
      <SidebarToggle />
      <SelectedTitle />
      {layout.isCompactHeader && <SessionActivity />}
      <div id="conversation-header-tools" />
      <HeaderButtons />
      {!layout.isCompactHeader && <SessionActivity />}
      {!navigation.selectedRoom?.radio && (
        <Menu
          position="bottom-end"
          withinPortal
          withInitialFocusPlaceholder={!layout.isCompactHeader}
        >
          <Menu.Target>
            <ActionIcon aria-label="Chat actions" title="Chat actions">
              <MoreHorizontal size={18} />
            </ActionIcon>
          </Menu.Target>
          <Menu.Dropdown className="chat-actions-menu">
            <ChatActionsMenu />
          </Menu.Dropdown>
        </Menu>
      )}
    </header>
  );
});
const SessionActivity = observer(function SessionActivity() {
  const { navigation, layout, ui } = useConversationActions();
  return navigation.selectedAgent ? (
    <LiveSessionActivity
      agentId={navigation.selectedAgent.id}
      compact={layout.isCompactHeader}
      onProcess={ui.openActivity}
    />
  ) : null;
});

import { reportPromptComposerRender } from "../../components/prompt-composer/renderProbe.ts";
import { observer } from "mobx-react-lite";
import { ActionIcon, Menu } from "@mantine/core";
import { MoreHorizontal } from "lucide-react";
import { SidebarToggle, HeaderButtons } from "../shell/NavigationButtons.tsx";
import { LiveSessionActivity } from "../../components/agents/SessionActivity.tsx";
import { SelectedTitle } from "./SelectedTitle.tsx";
import { ChatActionsMenu } from "./ChatActionsMenu.tsx";
import { useConversationActions } from "../../components/conversation/navigation.ts";

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

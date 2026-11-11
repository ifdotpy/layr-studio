import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import {
  Activity,
  ArrowLeft,
  BookOpen,
  Clock3,
  FileDiff,
  Layers,
  Search,
  Settings,
  Square,
  Terminal,
  Users,
} from "lucide-react";
import { NativeModelActions } from "../NativeModelActions/NativeModelActions.tsx";
import { MarkChatUnreadItem } from "../MarkChatUnreadItem/MarkChatUnreadItem.tsx";
import { ConversationMenuTools } from "../ConversationMenuTools/ConversationMenuTools.tsx";
import { TeamCount } from "../../agents/TeamCount/TeamCount.tsx";
import { TeamPresence } from "../../agents/TeamPresence/TeamPresence.tsx";
import { useConversationActions } from "../support/navigation.ts";

export const ChatActionsMenu = observer(function ChatActionsMenu() {
  const area = useConversationActions();
  const { ui, settings } = area;
  const { selected, layout } = area;
  return (
    <>
      {layout.usesMenuHeaderTools && <ConversationMenuTools />}
      {selected.canReturnToLead && (
        <Menu.Item
          aria-label="Back to main agent"
          leftSection={<ArrowLeft size={14} />}
          onClick={selected.returnToLead}
        >
          Back to main agent
        </Menu.Item>
      )}
      <Menu.Label>Views</Menu.Label>
      {(
        [
          ["changes", "Changes", FileDiff],
          ["plan", "Plan", BookOpen],
          ["rules", "Wake rules", Clock3],
          ["search", "Search", Search],
        ] as const
      ).map(([section, label, Icon]) => (
        <Menu.Item
          key={section}
          aria-label={section === "rules" ? "Rules" : label}
          data-workspace-section={section}
          leftSection={<Icon size={14} />}
          onClick={() => ui.showSection(section)}
        >
          {label}
        </Menu.Item>
      ))}
      <Menu.Item
        id="tasks-toggle"
        aria-label={selected.backgroundActivityLabel}
        leftSection={<Activity size={14} />}
        onClick={() => ui.openActivity()}
      >
        Background activity {selected.backgroundTaskCount || ""}
      </Menu.Item>
      {selected.canShowTerminalMenu && (
        <Menu.Item
          aria-label="Terminals"
          leftSection={<Terminal size={14} />}
          onClick={selected.toggleTerminal}
        >
          Terminals
        </Menu.Item>
      )}
      <Menu.Divider />
      <Menu.Label>Chat</Menu.Label>
      <Menu.Item
        id="chat-actions-settings"
        aria-label="Chat settings"
        leftSection={<Settings size={14} />}
        onClick={() => settings.setChatOpen(true)}
      >
        Chat settings
      </Menu.Item>
      <TeamPresence>
        <Menu.Item
          id="chat-actions-team"
          aria-label="Team"
          leftSection={<Users size={14} />}
          onClick={() => ui.toggleTeam()}
        >
          Team <TeamCount />
        </Menu.Item>
      </TeamPresence>
      <ManagedActions />
    </>
  );
});
const ManagedActions = observer(function ManagedActions() {
  const area = useConversationActions();
  const { selected, navigation } = area;
  return selected.isManagedConversation && navigation.selectedAgent ? (
    <>
      <MarkChatUnreadItem agent={navigation.selectedAgent} />
      {selected.canStopTeam && (
        <Menu.Item
          data-action="stop-team"
          color="red"
          leftSection={<Square size={14} />}
          onClick={selected.stopTeam}
        >
          Stop team
        </Menu.Item>
      )}
      {selected.canSwitchToLayr && (
        <Menu.Item
          data-action="switch-to-layr"
          leftSection={<Layers size={14} />}
          disabled={selected.isWorkspaceSwitchBlocked}
          onClick={selected.switchToLayr}
        >
          Switch to Layr
          <small className="menu-action-help">
            Keeps this chat and its history. Commit and push worktree changes
            first.
          </small>
        </Menu.Item>
      )}
      <Menu.Divider />
      <Menu.Label>Model actions</Menu.Label>
      <NativeModelActions agent={navigation.selectedAgent} />
    </>
  ) : null;
});

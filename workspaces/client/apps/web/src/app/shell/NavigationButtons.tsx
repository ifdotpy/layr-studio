import { ActionIcon, Button } from "@mantine/core";
import {
  ArrowLeft,
  MessageSquare,
  PanelLeft,
  Settings2,
  Users,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import type { ReactNode } from "react";
import { EdgeRail } from "../../components/edge-rail/EdgeRail.tsx";
import {
  TeamCount,
  TeamPresence,
} from "../../components/agents/TeamPanelContext.tsx";
import { useServerAreas as useNavigationButtons } from "../ServerAreasProvider.tsx";
export const SidebarToggle = observer(function SidebarToggle() {
  const { sidebar, layout } = useNavigationButtons();
  return layout.isMobile ? (
    <Button
      variant="subtle"
      size="compact-xs"
      id="sidebar-toggle"
      aria-label="Back to chats"
      aria-expanded={sidebar.isExpanded}
      onClick={sidebar.toggle}
      leftSection={<ArrowLeft size={18} />}
    >
      Chats
    </Button>
  ) : (
    <ActionIcon
      id="sidebar-toggle"
      aria-label="Toggle conversations"
      aria-expanded={sidebar.isExpanded}
      onClick={sidebar.toggle}
    >
      <PanelLeft size={18} />
    </ActionIcon>
  );
});
export const HeaderButtons = observer(function HeaderButtons() {
  const { layout, navigation, ui, settings } = useNavigationButtons();
  const panels = ui,
    workspace = ui;
  const radio = navigation.selectedRoom?.radio;
  return (
    <>
      {!layout.isCompactHeader && !radio && (
        <ActionIcon
          id="chat-settings-toggle"
          aria-label="Chat settings"
          title="Chat settings"
          onClick={() => settings.setChatOpen(true)}
        >
          <Settings2 size={18} />
        </ActionIcon>
      )}
      {!layout.isCompactHeader && (
        <TeamPresence>
          <Button
            variant="subtle"
            leftSection={<Users size={16} />}
            id="team-toggle"
            aria-label="Team"
            aria-expanded={panels.isTeamVisible}
            onClick={panels.toggleTeam}
          >
            Team
            <TeamCount className="team-button-count" />
          </Button>
        </TeamPresence>
      )}
      {!radio && (!layout.isCompactHeader || workspace.attentionCount > 0) && (
        <Button
          variant="subtle"
          id="messages-toggle"
          aria-label="Messages"
          title={
            workspace.attentionCount > 0
              ? `Messages, ${workspace.attentionCount} need you`
              : "Messages"
          }
          leftSection={<MessageSquare size={16} />}
          disabled={!navigation.selectedLead}
          onClick={() => workspace.showMessages()}
        >
          <span className="messages-toggle-label">Messages</span>
          {workspace.attentionCount > 0 && (
            <span className="attention-count">{workspace.attentionCount}</span>
          )}
        </Button>
      )}
    </>
  );
});
export const RailRoot = observer(function RailRoot({
  menu,
}: {
  menu: ReactNode;
}) {
  const { layout, sidebar, navigation, ui } = useNavigationButtons();
  const workspace = ui;
  return (
    <EdgeRail
      geometry={layout.railGeometry}
      sidebar={{
        id: "sidebar-toggle",
        label: sidebar.toggleLabel,
        expanded: sidebar.isExpanded,
        onClick: sidebar.toggle,
      }}
      messages={{
        count: workspace.attentionCount,
        disabled: !navigation.selectedLead,
        onClick: () => workspace.showMessages(),
      }}
      menu={menu}
    />
  );
});

import { ActionIcon, Button } from "@mantine/core";
import { MessageSquare, Settings2, Users } from "lucide-react";
import { observer } from "mobx-react-lite";
import { TeamCount } from "../../agents/TeamCount/TeamCount.tsx";
import { TeamPresence } from "../../agents/TeamPresence/TeamPresence.tsx";
import { useServerAreas as useNavigationButtons } from "../../../app/ServerAreasProvider.tsx";

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

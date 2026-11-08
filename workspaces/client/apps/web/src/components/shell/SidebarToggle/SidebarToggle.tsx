import { ActionIcon, Button } from "@mantine/core";
import { ArrowLeft, PanelLeft } from "lucide-react";
import { observer } from "mobx-react-lite";
import { useServerAreas as useNavigationButtons } from "../../../app/ServerAreasProvider.tsx";

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

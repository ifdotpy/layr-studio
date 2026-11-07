import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { FolderOpen } from "lucide-react";
import type { SidebarChatView } from "../sidebar/viewModel.ts";
import type { ChatRowActions } from "./actions.ts";

export const ProjectAction = observer(function ProjectAction({
  row,
  disabled,
  compact,
  actions,
}: {
  row: SidebarChatView;
  disabled: boolean;
  compact: boolean;
  actions: ChatRowActions;
}) {
  const a = row;
  return (
    !compact && (
      <Menu.Item
        leftSection={<FolderOpen size={14} />}
        disabled={disabled || !a.canChangeProject}
        onClick={() => actions.changeProject()}
      >
        Change project directory
      </Menu.Item>
    )
  );
});

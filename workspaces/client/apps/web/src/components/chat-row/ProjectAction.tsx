import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { FolderOpen } from "lucide-react";
import type { SidebarChatView } from "../sidebar/viewModel.ts";
import {
  useChatRowActions,
  useSidebarRowState,
  useSidebarRowActions,
} from "./actions.ts";

export const ProjectAction = observer(function ProjectAction({
  row,
}: {
  row: SidebarChatView;
}) {
  const area = useSidebarRowActions();
  const disabled = useSidebarRowState(row).disabled;
  const actions = useChatRowActions(row);
  const a = row;
  return (
    !area.ui.compact && (
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

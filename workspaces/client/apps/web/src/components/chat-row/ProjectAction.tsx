import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { FolderOpen } from "lucide-react";
import { type ChatRowProps, useSidebarRowActions } from "./actions.ts";

export const ProjectAction = observer(function ProjectAction({
  state,
}: ChatRowProps) {
  const row = state.row;
  const area = useSidebarRowActions();
  const disabled = state.disabled;
  const actions = state.actions;
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

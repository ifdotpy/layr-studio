import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Folder } from "lucide-react";
import { type ChatRowProps } from "./actions.ts";

export const MoveAction = observer(function MoveAction({
  state,
}: ChatRowProps) {
  const row = state.row;
  const disabled = state.disabled;
  const actions = state.actions;
  return (
    row.projectPath && (
      <Menu.Item
        disabled={disabled}
        leftSection={<Folder size={14} />}
        onClick={actions.move}
      >
        Move to folder
      </Menu.Item>
    )
  );
});

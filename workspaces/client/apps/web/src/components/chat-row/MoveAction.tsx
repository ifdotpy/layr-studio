import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Folder } from "lucide-react";
import type { SidebarChatView } from "../sidebar/viewModel.ts";
import type { ChatRowActions } from "./actions.ts";

export const MoveAction = observer(function MoveAction({
  row,
  disabled,
  actions,
}: {
  row: SidebarChatView;
  disabled: boolean;
  actions: ChatRowActions;
}) {
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

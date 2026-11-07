import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Folder } from "lucide-react";
import type { SidebarChatView } from "../sidebar/viewModel.ts";
import { useChatRowActions } from "./actions.ts";

export const MoveAction = observer(function MoveAction({
  row,
  disabled,
}: {
  row: SidebarChatView;
  disabled: boolean;
}) {
  const actions = useChatRowActions(row);
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

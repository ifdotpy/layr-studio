import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Folder } from "lucide-react";
import type { SidebarChatRow } from "../../generated/observableResources.ts";
import type { ChatRowActions } from "./actions.ts";

export const MoveAction = observer(function MoveAction({
  row,
  disabled,
  actions,
}: {
  row: SidebarChatRow;
  disabled: boolean;
  actions: ChatRowActions;
}) {
  const a = row;
  return (
    a.projectPath && (
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

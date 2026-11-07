import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Archive, ArchiveRestore } from "lucide-react";
import type { SidebarChatView } from "../sidebar/viewModel.ts";
import type { ChatRowActions } from "./actions.ts";

export const ArchiveAction = observer(function ArchiveAction({
  row,
  disabled,
  actions,
}: {
  row: SidebarChatView;
  disabled: boolean;
  actions: ChatRowActions;
}) {
  return (
    <Menu.Item
      leftSection={
        row.agent.archived ? (
          <ArchiveRestore size={14} />
        ) : (
          <Archive size={14} />
        )
      }
      disabled={disabled || !row.canArchive}
      onClick={() => actions.archive()}
    >
      {row.agent.archived ? "Restore chat" : "Archive"}
    </Menu.Item>
  );
});

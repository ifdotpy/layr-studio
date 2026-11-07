import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Archive, ArchiveRestore } from "lucide-react";
import type { SidebarChatRow } from "../../generated/observableResources.ts";
import type { ChatRowActions } from "./actions.ts";

export const ArchiveAction = observer(function ArchiveAction({
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
    <Menu.Item
      leftSection={
        a.archived ? <ArchiveRestore size={14} /> : <Archive size={14} />
      }
      disabled={disabled || !a.canArchive}
      onClick={() => actions.archive()}
    >
      {a.archived ? "Restore chat" : "Archive"}
    </Menu.Item>
  );
});

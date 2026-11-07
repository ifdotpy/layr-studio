import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Archive, ArchiveRestore } from "lucide-react";
import { type ChatRowProps } from "./actions.ts";

export const ArchiveAction = observer(function ArchiveAction({
  state,
}: ChatRowProps) {
  const row = state.row;
  const disabled = state.disabled;
  const actions = state.actions;
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

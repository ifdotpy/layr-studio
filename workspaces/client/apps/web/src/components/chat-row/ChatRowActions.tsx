import { ArchiveAction } from "./ArchiveAction.tsx";
import { ProjectAction } from "./ProjectAction.tsx";
import { MoveAction } from "./MoveAction.tsx";
import { PinAction } from "./PinAction.tsx";
import { UnreadAction } from "./UnreadAction.tsx";
import { forwardRef, type ComponentPropsWithoutRef } from "react";
import { observer } from "mobx-react-lite";
import { ActionIcon, Button, Menu, TextInput } from "@mantine/core";
import { MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import type { SidebarChatView } from "../sidebar/viewModel.ts";

import { useChatRowActions, useSidebarRowState } from "./actions.ts";

const RowActionTarget = observer(
  forwardRef<
    HTMLButtonElement,
    { row: SidebarChatView } & ComponentPropsWithoutRef<typeof ActionIcon>
  >(function RowActionTarget({ row, ...props }, ref) {
    return (
      <ActionIcon
        {...props}
        ref={ref}
        className="row-actions"
        aria-label={`Actions for ${row.agent.name}`}
      >
        <MoreHorizontal size={16} />
      </ActionIcon>
    );
  }),
);

export const ChatRowActionMenu = observer(function ChatRowActionMenu({
  row,
  priority = false,
}: {
  row: SidebarChatView;
  priority?: boolean;
}) {
  const state = useSidebarRowState(row, priority);
  const disabled = useSidebarRowState(row).disabled;
  const actions = useChatRowActions(row, priority);
  return state.renaming ? (
    <form className="inline-rename" onSubmit={(e) => actions.rename(e)}>
      <TextInput
        aria-label="Chat name"
        value={state.name}
        onChange={(e) => actions.name(e.target.value)}
        autoFocus
        maxLength={80}
        onKeyDown={(e) => {
          if (e.key === "Escape") actions.cancelRename();
        }}
      />
      <Button type="submit" aria-label="Save name" disabled={disabled}>
        Save
      </Button>
      <Button onClick={() => actions.cancelRename()}>Cancel</Button>
    </form>
  ) : (
    <Menu position="bottom-end" withinPortal shadow="lg" width={220}>
      <Menu.Target>
        <RowActionTarget row={row} />
      </Menu.Target>
      <Menu.Dropdown className="fleet-menu">
        <Menu.Item
          disabled={disabled}
          leftSection={<Pencil size={14} />}
          onClick={actions.beginRename}
        >
          Rename
        </Menu.Item>
        <Menu.Divider />
        <UnreadAction row={row} />
        <PinAction row={row} />
        <Menu.Divider />
        <MoveAction row={row} />
        <ProjectAction row={row} />
        <Menu.Divider />
        <ArchiveAction row={row} />
        <Menu.Divider />
        <Menu.Item
          disabled={disabled}
          color="red"
          leftSection={<Trash2 size={14} />}
          onClick={() => actions.remove()}
        >
          Delete
        </Menu.Item>
      </Menu.Dropdown>
    </Menu>
  );
});

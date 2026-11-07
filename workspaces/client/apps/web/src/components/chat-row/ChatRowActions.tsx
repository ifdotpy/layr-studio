import { ArchiveAction } from "./ArchiveAction.tsx";
import { ProjectAction } from "./ProjectAction.tsx";
import { MoveAction } from "./MoveAction.tsx";
import { PinAction } from "./PinAction.tsx";
import { UnreadAction } from "./UnreadAction.tsx";
import { forwardRef, type ComponentPropsWithoutRef } from "react";
import { observer } from "mobx-react-lite";
import { ActionIcon, Button, Menu, TextInput } from "@mantine/core";
import { MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import type { SidebarChatRow } from "../../generated/observableResources.ts";

import type { ChatRowActions } from "./actions.ts";

const RowActionTarget = observer(
  forwardRef<
    HTMLButtonElement,
    { row: SidebarChatRow } & ComponentPropsWithoutRef<typeof ActionIcon>
  >(function RowActionTarget({ row, ...props }, ref) {
    return (
      <ActionIcon
        {...props}
        ref={ref}
        className="row-actions"
        aria-label={`Actions for ${row.name}`}
      >
        <MoreHorizontal size={16} />
      </ActionIcon>
    );
  }),
);

export const ChatRowActionMenu = observer(function ChatRowActionMenu({
  row,
  pinned,
  renaming,
  name,
  disabled = false,
  organizing,
  compact,
  markingRead,
  actions,
}: {
  row: SidebarChatRow;
  pinned?: boolean | undefined;
  renaming: boolean;
  name: string;
  disabled?: boolean | undefined;
  organizing: boolean;
  compact: boolean;
  markingRead: boolean;
  actions: ChatRowActions;
}) {
  return renaming ? (
    <form className="inline-rename" onSubmit={(e) => actions.rename(e)}>
      <TextInput
        aria-label="Chat name"
        value={name}
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
        <UnreadAction
          row={row}
          disabled={disabled}
          markingRead={markingRead}
          actions={actions}
        />
        <PinAction
          row={row}
          pinned={pinned}
          disabled={disabled}
          organizing={organizing}
          actions={actions}
        />
        <Menu.Divider />
        <MoveAction row={row} disabled={disabled} actions={actions} />
        <ProjectAction
          row={row}
          disabled={disabled}
          compact={compact}
          actions={actions}
        />
        <Menu.Divider />
        <ArchiveAction row={row} disabled={disabled} actions={actions} />
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

import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Pin, PinOff } from "lucide-react";
import type { SidebarChatRow } from "../../generated/observableResources.ts";
import type { ChatRowActions } from "./actions.ts";

export const PinAction = observer(function PinAction({
  row,
  pinned,
  disabled,
  organizing,
  actions,
}: {
  row: SidebarChatRow;
  pinned: boolean | undefined;
  disabled: boolean;
  organizing: boolean;
  actions: ChatRowActions;
}) {
  const a = row;
  return (
    <Menu.Item
      disabled={disabled || organizing}
      leftSection={
        (pinned ?? a.pinned) ? <PinOff size={14} /> : <Pin size={14} />
      }
      onClick={() => actions.pin()}
    >
      {(pinned ?? a.pinned) ? "Unpin" : "Pin"}
    </Menu.Item>
  );
});

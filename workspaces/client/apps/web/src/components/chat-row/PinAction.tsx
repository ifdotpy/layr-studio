import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Pin, PinOff } from "lucide-react";
import type { SidebarChatView } from "../sidebar/viewModel.ts";
import { useChatRowActions } from "./actions.ts";

export const PinAction = observer(function PinAction({
  row,
  pinned,
  disabled,
  organizing,
}: {
  row: SidebarChatView;
  pinned: boolean | undefined;
  disabled: boolean;
  organizing: boolean;
}) {
  const actions = useChatRowActions(row);
  return (
    <Menu.Item
      disabled={disabled || organizing}
      leftSection={
        (pinned ?? row.agent.pinned) ? <PinOff size={14} /> : <Pin size={14} />
      }
      onClick={() => actions.pin()}
    >
      {(pinned ?? row.agent.pinned) ? "Unpin" : "Pin"}
    </Menu.Item>
  );
});

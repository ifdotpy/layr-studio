import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Pin, PinOff } from "lucide-react";
import type { SidebarChatView } from "../sidebar/viewModel.ts";
import { useChatRowActions, useSidebarRowState } from "./actions.ts";

export const PinAction = observer(function PinAction({
  row,
}: {
  row: SidebarChatView;
}) {
  const disabled = useSidebarRowState(row).disabled;
  const state = useSidebarRowState(row);
  const actions = useChatRowActions(row);
  return (
    <Menu.Item
      disabled={disabled || state.organizing}
      leftSection={
        (state.pinned ?? row.agent.pinned) ? (
          <PinOff size={14} />
        ) : (
          <Pin size={14} />
        )
      }
      onClick={() => actions.pin()}
    >
      {(state.pinned ?? row.agent.pinned) ? "Unpin" : "Pin"}
    </Menu.Item>
  );
});

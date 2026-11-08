import { observer } from "mobx-react-lite";
import { Menu } from "@mantine/core";
import { Pin, PinOff } from "lucide-react";
import type { ChatRowProps } from "../support/actions.ts";

export const PinAction = observer(function PinAction({ state }: ChatRowProps) {
  const row = state.row;
  const disabled = state.disabled;
  const actions = state.actions;
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

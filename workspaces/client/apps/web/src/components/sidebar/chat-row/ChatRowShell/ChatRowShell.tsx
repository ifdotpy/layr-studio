import type { PropsWithChildren } from "react";
import { observer } from "mobx-react-lite";
import { displayedIndicator } from "../support/indicator.ts";
import type { ChatRowProps } from "../support/actions.ts";

export const ChatRowShell = observer(function ChatRowShell({
  state,
  children,
}: PropsWithChildren<ChatRowProps>) {
  const row = state.row;
  return (
    <div
      className={`sidebar-row fleet-strip lead-row ${state.selected ? "selected" : ""}`}
      data-sidebar-item={row.id}
      data-studio-state={displayedIndicator(row, state.pendingRead).kind}
    >
      {children}
    </div>
  );
});

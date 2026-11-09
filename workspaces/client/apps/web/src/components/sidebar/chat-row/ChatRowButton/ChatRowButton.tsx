import type { PropsWithChildren } from "react";
import { observer } from "mobx-react-lite";
import { UnstyledButton } from "@mantine/core";
import { displayedIndicator } from "../support/indicator.ts";
import { offlineServerDescription } from "../ChatServerLine/ChatServerLine.tsx";
import type { ChatRowProps } from "../support/actions.ts";

export const ChatRowButton = observer(function ChatRowButton({
  state,
  children,
}: PropsWithChildren<ChatRowProps>) {
  const row = state.row;
  const server = state.server;
  return (
    <UnstyledButton
      {...state.bindings}
      data-drop-edge={state.dropEdge}
      title={row.agent.name ?? undefined}
      aria-label={
        [row.agent.name, server.show ? `Server ${server.alias}` : undefined]
          .filter(Boolean)
          .join(" ") || undefined
      }
      aria-description={[
        "Drag onto a team to join it, or onto the project name to leave. Drag to an edge to reorder. Alt + Up or Down also works.",
        server.offline ? offlineServerDescription(server.alias) : undefined,
        displayedIndicator(row, state.pendingRead).label,
      ]
        .filter(Boolean)
        .join(" ")}
      className="chat-row"
      data-chat={row.id}
      onClick={state.actions.open}
      aria-current={state.selected}
    >
      {children}
    </UnstyledButton>
  );
});

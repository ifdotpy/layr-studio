import type { PropsWithChildren } from "react";
import { observer } from "mobx-react-lite";
import { UnstyledButton } from "@mantine/core";
import { displayedIndicator } from "./indicator.ts";
import { offlineServerDescription } from "./ChatServerLine.tsx";
import { type ChatRowProps } from "./actions.ts";

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
      data-folder-drop={state.folderDrop}
      title={row.agent.name ?? undefined}
      aria-label={
        [row.agent.name, server.show ? `Server ${server.alias}` : undefined]
          .filter(Boolean)
          .join(" ") || undefined
      }
      aria-description={[
        "Drag onto a team to join it, or onto the project name to leave. Drop a team chat in the center of another lead chat to make it a subagent. Drag to an edge to reorder. Alt + Up or Down also works.",
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

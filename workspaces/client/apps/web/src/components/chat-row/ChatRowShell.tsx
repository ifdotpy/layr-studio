import type { HTMLAttributes, ReactNode } from "react";
import { observer } from "mobx-react-lite";
import { UnstyledButton } from "@mantine/core";
import type { SidebarChatView, SidebarAgent } from "../sidebar/viewModel.ts";
import { displayedIndicator } from "./indicator.ts";
import { offlineServerDescription } from "./ChatServerLine.tsx";
import type { ChatRowActions } from "./actions.ts";

export const ChatRowShell = observer(function ChatRowShell({
  row,
  selected,
  pendingRead,
  children,
}: {
  row: SidebarChatView;
  selected: boolean;
  pendingRead?: SidebarAgent["markRead"] | undefined;
  children: ReactNode;
}) {
  return (
    <div
      className={`sidebar-row fleet-strip lead-row ${selected ? "selected" : ""}`}
      data-sidebar-item={row.id}
      data-studio-state={displayedIndicator(row, pendingRead).kind}
    >
      {children}
    </div>
  );
});

export const ChatRowButton = observer(function ChatRowButton({
  row,
  selected,
  pendingRead,
  serverOffline,
  serverAlias,
  bindings,
  events,
  actions,
  children,
}: {
  row: SidebarChatView;
  selected: boolean;
  pendingRead?: SidebarAgent["markRead"] | undefined;
  serverOffline: boolean;
  serverAlias: string;
  bindings: HTMLAttributes<HTMLElement>;
  events: HTMLAttributes<HTMLElement>;
  actions: ChatRowActions;
  children: ReactNode;
}) {
  return (
    <UnstyledButton
      {...bindings}
      {...events}
      title={row.agent.name}
      aria-description={[
        "Drag onto a team to join it, or onto the project name to leave. Drop a team chat in the center of another lead chat to make it a subagent. Drag to an edge to reorder. Alt + Up or Down also works.",
        serverOffline ? offlineServerDescription(serverAlias) : undefined,
        displayedIndicator(row, pendingRead).label,
      ]
        .filter(Boolean)
        .join(" ")}
      className="chat-row"
      data-chat={row.id}
      onPointerEnter={actions.prepare}
      onFocus={actions.prepare}
      onPointerDown={actions.prepare}
      onClick={actions.open}
      aria-current={selected}
    >
      {children}
    </UnstyledButton>
  );
});

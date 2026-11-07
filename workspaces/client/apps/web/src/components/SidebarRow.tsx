import type { HTMLAttributes } from "react";
import { observer } from "mobx-react-lite";
import type { SidebarChatView, SidebarAgent } from "./sidebar/viewModel.ts";
import { reportPromptComposerRender } from "./prompt-composer/renderProbe";
import {
  ChatTitle,
  ChatIndicator,
  ChatCopyProgress,
} from "./chat-row/index.ts";
import { ChatRowActionMenu } from "./chat-row/ChatRowActions.tsx";
import { ChatRowShell, ChatRowButton } from "./chat-row/ChatRowShell.tsx";
import { ChatRowMeta } from "./chat-row/ChatRowMeta.tsx";

type Bindings = HTMLAttributes<HTMLElement> & {
  "data-folder-drop"?: string | undefined;
  "data-sidebar-group"?: string | undefined;
  "data-sidebar-id"?: string | undefined;
  "data-drop-edge"?: string | undefined;
};
type Props = {
  disabled?: boolean | undefined;
  row: SidebarChatView;
  selected: boolean;
  serverAlias: string;
  showServer: boolean;
  pendingRead?: SidebarAgent["markRead"] | undefined;
  pinned?: boolean | undefined;
  serverOffline?: boolean | undefined;
  renaming: boolean;
  organizing: boolean;
  name: string;
  compact: boolean;
  markingRead: boolean;
  bindings: Bindings;
  priority?: boolean;
};

export default observer(function SidebarRow({
  row,
  selected,
  serverAlias,
  showServer,
  pendingRead,
  pinned,
  serverOffline = false,
  renaming,
  organizing,
  name,
  compact,
  markingRead,
  disabled = false,
  bindings,
  priority = false,
}: Props) {
  reportPromptComposerRender("sidebar-row", row.id);
  return (
    <ChatRowShell row={row} selected={selected} pendingRead={pendingRead}>
      <ChatRowButton
        row={row}
        selected={selected}
        pendingRead={pendingRead}
        serverOffline={serverOffline}
        serverAlias={serverAlias}
        bindings={bindings}
      >
        <span className="row-copy">
          <span className="fleet-chat-heading">
            <ChatIndicator row={row} pendingRead={pendingRead} />
            <ChatTitle row={row} pinned={pinned} />
          </span>
          <ChatCopyProgress row={row} />
          <ChatRowMeta
            row={row}
            showServer={showServer}
            serverOffline={serverOffline}
            serverAlias={serverAlias}
          />
        </span>
      </ChatRowButton>
      <ChatRowActionMenu
        row={row}
        pinned={pinned}
        renaming={renaming}
        name={name}
        disabled={disabled}
        organizing={organizing}
        compact={compact}
        markingRead={markingRead}
        priority={priority}
      />
    </ChatRowShell>
  );
});

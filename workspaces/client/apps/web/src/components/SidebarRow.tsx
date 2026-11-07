import { observer } from "mobx-react-lite";
import type { SidebarChatView } from "./sidebar/viewModel.ts";
import { reportPromptComposerRender } from "./prompt-composer/renderProbe";
import {
  ChatTitle,
  ChatIndicator,
  ChatCopyProgress,
} from "./chat-row/index.ts";
import { ChatRowActionMenu } from "./chat-row/ChatRowActions.tsx";
import { ChatRowShell, ChatRowButton } from "./chat-row/ChatRowShell.tsx";
import { ChatRowMeta } from "./chat-row/ChatRowMeta.tsx";
import { useSidebarRowState } from "./chat-row/actions.ts";

const SidebarRowIdentity = observer(function SidebarRowIdentity({
  row,
}: {
  row: SidebarChatView;
}) {
  const state = useSidebarRowState(row);
  return (
    <span className="fleet-chat-heading">
      <ChatIndicator row={row} pendingRead={state.pendingRead} />
      <ChatTitle row={row} pinned={state.pinned} />
    </span>
  );
});

export default observer(function SidebarRow({
  row,
  priority = false,
}: {
  row: SidebarChatView;
  priority?: boolean;
}) {
  reportPromptComposerRender("sidebar-row", row.id);
  return (
    <ChatRowShell row={row}>
      <ChatRowButton row={row}>
        <span className="row-copy">
          <SidebarRowIdentity row={row} />
          <ChatCopyProgress row={row} />
          <ChatRowMeta row={row} />
        </span>
      </ChatRowButton>
      <ChatRowActionMenu row={row} priority={priority} />
    </ChatRowShell>
  );
});

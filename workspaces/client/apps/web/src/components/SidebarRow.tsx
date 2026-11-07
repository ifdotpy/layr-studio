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
import { useSidebarRowState, type ChatRowProps } from "./chat-row/actions.ts";

const SidebarRowIdentity = observer(function SidebarRowIdentity({
  state,
}: ChatRowProps) {
  return (
    <span className="fleet-chat-heading">
      <ChatIndicator row={state.row} pendingRead={state.pendingRead} />
      <ChatTitle row={state.row} pinned={state.pinned} />
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
  const state = useSidebarRowState(row, priority);
  reportPromptComposerRender("sidebar-row", row.id);
  return (
    <ChatRowShell state={state}>
      <ChatRowButton state={state}>
        <span className="row-copy">
          <SidebarRowIdentity state={state} />
          <ChatCopyProgress row={row} />
          <ChatRowMeta state={state} />
        </span>
      </ChatRowButton>
      <ChatRowActionMenu state={state} />
    </ChatRowShell>
  );
});

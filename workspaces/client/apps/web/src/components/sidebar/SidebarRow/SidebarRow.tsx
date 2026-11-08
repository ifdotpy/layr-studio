import { observer } from "mobx-react-lite";
import type { SidebarChatView } from "../support/viewModel.ts";
import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
import { ChatTitle } from "../chat-row/ChatTitle/ChatTitle.tsx";
import { default as ChatIndicator } from "../../agents/ChatStatus/ChatStatus.tsx";
import { default as ChatCopyProgress } from "../LayrCopyProgress/LayrCopyProgress.tsx";
import { ChatRowActionMenu } from "../chat-row/ChatRowActionMenu/ChatRowActionMenu.tsx";
import { ChatRowShell } from "../chat-row/ChatRowShell/ChatRowShell.tsx";
import { ChatRowButton } from "../chat-row/ChatRowButton/ChatRowButton.tsx";
import { ChatRowMeta } from "../chat-row/ChatRowMeta/ChatRowMeta.tsx";
import {
  useSidebarRowState,
  type ChatRowProps,
} from "../chat-row/support/actions.ts";

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

import { observer } from "mobx-react-lite";
import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
import { default as ChatIndicator } from "../../agents/ChatStatus/ChatStatus.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { default as ChatCopyProgress } from "../LayrCopyProgress/LayrCopyProgress.tsx";
import { ChatRowActionMenu } from "../chat-row/ChatRowActionMenu/ChatRowActionMenu.tsx";
import { ChatRowButton } from "../chat-row/ChatRowButton/ChatRowButton.tsx";
import { ChatRowMeta } from "../chat-row/ChatRowMeta/ChatRowMeta.tsx";
import { ChatRowShell } from "../chat-row/ChatRowShell/ChatRowShell.tsx";
import { ChatTitle } from "../chat-row/ChatTitle/ChatTitle.tsx";
import {
  createSidebarRowState,
  useSidebarRowActions,
  type ChatRowProps,
} from "../chat-row/support/actions.ts";
import type { SidebarChatView } from "../support/viewModel.ts";

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
  const area = useSidebarRowActions();
  reportPromptComposerRender("sidebar-row", row.id);
  return (
    <Owned
      key={`${area.viewKey}:${row.id}:${priority}`}
      create={() => createSidebarRowState(area, row, priority)}
    >
      {(state) => (
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
      )}
    </Owned>
  );
});

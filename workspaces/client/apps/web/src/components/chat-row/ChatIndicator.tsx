import { observer } from "mobx-react-lite";
import { ChatOperationalKind } from "../../generated/api.ts";
import type { SidebarChatRow } from "../../generated/observableResources.ts";
import ChatStatus from "../agents/ChatStatus.tsx";
import { displayedIndicator, type IndicatorRow } from "./indicator.ts";

export const ChatIndicator = observer(function ChatIndicator({
  row,
  model,
  pendingRead,
}: {
  row: IndicatorRow;
  model?: string | undefined;
  pendingRead?: SidebarChatRow["markRead"] | undefined;
}) {
  const status = displayedIndicator(row, pendingRead);
  // Read tracked fields before passing the display snapshot.
  const kind = status.kind;
  switch (kind) {
    case ChatOperationalKind.none:
    case ChatOperationalKind.paused:
      return null;
    case ChatOperationalKind.working:
    case ChatOperationalKind.unread:
    case ChatOperationalKind.answer:
    case ChatOperationalKind.error:
      return (
        <ChatStatus
          status={{ kind, label: status.label }}
          provider={row.provider}
          model={model ?? row.model ?? undefined}
        />
      );
  }
  return kind satisfies never;
});

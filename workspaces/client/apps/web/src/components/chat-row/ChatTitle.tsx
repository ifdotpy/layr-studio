import { observer } from "mobx-react-lite";
import { Pin } from "lucide-react";
import type { SidebarChatRow } from "../../generated/observableResources.ts";

type Row = Pick<SidebarChatRow, "id" | "name"> & Partial<Pick<SidebarChatRow, "pinned">>;
const ChatPin = observer(function ChatPin({ row, pinned }: {row: Row; pinned?: boolean | undefined}) {
  return (pinned ?? row.pinned) ? <Pin size={11} className="chat-pin" /> : null;
});
export const ChatTitle = observer(function ChatTitle({ row, pinned }: {row: Row; pinned?: boolean | undefined}) {
  return <strong><ChatPin row={row} pinned={pinned} />{row.name}</strong>;
});

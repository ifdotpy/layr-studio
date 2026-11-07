import { observer } from "mobx-react-lite";
import { Pin } from "lucide-react";
import type { SidebarChatView, SidebarAgent } from "../sidebar/viewModel.ts";

type ChatProps = { row: SidebarChatView; pinned?: boolean | undefined };
const ChatPin = observer(function ChatPin({ row, pinned }: ChatProps) {
  return (pinned ?? row.agent.pinned) ? (
    <Pin size={11} className="chat-pin" />
  ) : null;
});
export const ChatTitle = observer(function ChatTitle(
  props:
    | ChatProps
    | { agent: SidebarAgent }
    | { room: { id: string; name: string } },
) {
  return "agent" in props ? (
    <strong title={props.agent.name || undefined}>{props.agent.name}</strong>
  ) : "row" in props ? (
    <strong>
      <ChatPin row={props.row} pinned={props.pinned} />
      {props.row.agent.name}
    </strong>
  ) : (
    <strong>{props.room.name}</strong>
  );
});

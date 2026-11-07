import { observer } from "mobx-react-lite";
import { Pin } from "lucide-react";
import type { SidebarChatView, SidebarAgent } from "../sidebar/viewModel.ts";
import type { ProjectChat } from "../../servers/ProjectChatRows.tsx";

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
    | { room: Pick<Extract<ProjectChat, { kind: "room" }>, "id" | "name"> },
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

import { createArea } from "../area.ts";
import type { ChatActivity } from "../chat-status/chatActivity.ts";
import type { useChatReadState } from "../useChatReadState.ts";

export type ConversationActions = {
  open(id: string): void;
  nativeAction(
    agent: import("../sidebar/viewModel.ts").SidebarAgent,
    action: "compact" | "review",
  ): void;
  openActivity(activity: ChatActivity): void;
  read: Pick<
    ReturnType<typeof useChatReadState>,
    "marking" | "markSidebarUnread" | "pendingReadTargets"
  >;
};
export const [ConversationProvider, useConversationActions] =
  createArea<ConversationActions>("Conversation");

import type { FormEvent } from "react";
import { createContext, useContext } from "react";
import type { SidebarChatView } from "../sidebar/viewModel.ts";
export type ChatRowActions = {
  prepare: () => void;
  open: () => void;
  pin: () => void;
  rename: (event: FormEvent) => void;
  name: (value: string) => void;
  cancelRename: () => void;
  beginRename: () => void;
  unread: () => void;
  move: () => void;
  changeProject: () => void;
  archive: () => void;
  remove: () => void;
};

export const ChatRowActionsContext = createContext<
  ((row: SidebarChatView, priority: boolean) => ChatRowActions) | null
>(null);
export function useChatRowActions(row: SidebarChatView, priority = false) {
  const actions = useContext(ChatRowActionsContext);
  if (!actions) throw new Error("Chat row actions are unavailable");
  return actions(row, priority);
}

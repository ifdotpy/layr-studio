import type { ServerCommand } from "./navigation.ts";
import type { useChatReadState } from "../components/useChatReadState.ts";

/** The App's frame-command receiver needs no Agent projection to apply a CAS. */
export function receiveSidebarRead(
  command: Extract<ServerCommand, { action: "mark-unread" }>,
  write: ReturnType<typeof useChatReadState>["markSidebarUnread"],
) {
  return write(command.id, { ...command.target });
}

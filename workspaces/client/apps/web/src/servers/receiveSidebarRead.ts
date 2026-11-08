import type { ServerCommand } from "./navigation.ts";
import type { ReadStateOwner } from "../app/conversation/ReadStateOwner.ts";

/** The App's frame-command receiver needs no Agent projection to apply a CAS. */
export function receiveSidebarRead(
  command: Extract<ServerCommand, { action: "mark-unread" }>,
  write: ReadStateOwner["markSidebarUnread"],
) {
  return write(command.id, { ...command.target });
}

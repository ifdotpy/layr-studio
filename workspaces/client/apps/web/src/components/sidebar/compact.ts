import { COMPACT_RECENT_WINDOW_SECONDS } from "../../generated/sharedValues.ts";
import type { SidebarChatRow } from "../../generated/observableResources.ts";

/** Server policy plus the render-time recency window. Crossing the boundary
 * takes effect at the next render; no timer or network interest is opened. */
export function keepCompactSidebarChat(
  row: Pick<SidebarChatRow, "compactPinned" | "activityAt">,
  now = Date.now() / 1000,
) {
  return row.compactPinned || now - row.activityAt < COMPACT_RECENT_WINDOW_SECONDS;
}

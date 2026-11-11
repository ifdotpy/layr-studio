import { AgentStatus, AgentSource } from "../../../generated/api.ts";
import type { Agent } from "../../../types.ts";
import type { SidebarAgent } from "../../sidebar/support/viewModel.ts";
type WaitAgent = Agent & Pick<SidebarAgent, "hasBackgroundWork">;

/** Detail membership consumes the child's checked wait; it never traverses or counts wake triggers. */
export function visibleWaitingChild(child: WaitAgent) {
  if (
    child.archived ||
    ["paused", "failed", "interrupted"].includes(child.status || "")
  )
    return false;
  if (
    child.inFlight ||
    ["queued", "starting", "running", "approval"].includes(child.status || "")
  )
    return true;
  if (child.status === AgentStatus["parked"] && child.parkedEvent) return true;
  if (
    child.autoWake === false ||
    !["waiting", "parked", "waiting_on_monitor"].includes(child.status || "")
  )
    return false;
  const status = child.operationalStatus;
  const wait =
    status?.reason === "waiting" || status?.reason === "needs_input"
      ? status.wait
      : null;
  return !!child.hasBackgroundWork || !!wait?.agents;
}

export function waitingChildren<T extends WaitAgent>(
  agents: Iterable<T>,
  parentId: string,
) {
  return [...agents].filter(
    (child) =>
      child.source === AgentSource["managed"] &&
      child.parentId === parentId &&
      child.id !== parentId &&
      visibleWaitingChild(child),
  );
}

import type { Agent } from "../../types.ts";
import type { ReadState } from "../../status/operationalStatus.ts";

/** Legacy receipt oracle retained for the independent #994 contract assertions. */
export function hasCompletedResult(agent: Agent) {
  return !!(
    agent.threadId &&
    agent.lastCompletedTurn &&
    agent.lastCompletedTurnStatus === "completed"
  );
}
export function unreadResult(
  agent: Agent,
  readState: ReadState | null = savedReadState(agent),
) {
  return (
    hasCompletedResult(agent) &&
    !(
      readState?.read === true &&
      readState.threadId === agent.threadId &&
      readState.turnId === agent.lastCompletedTurn
    )
  );
}

function savedReadState(agent: Agent): ReadState | null {
  return agent.readState ?? null;
}

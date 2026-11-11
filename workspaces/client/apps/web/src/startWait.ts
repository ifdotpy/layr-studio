import { AgentStatus, StartWaitReason } from "./generated/api.ts";
import type { Agent } from "./types.ts";

/** Render the server's epoch-scoped wait, shared with orchestration_status. */
export function startWaitLabel(
  agent: Pick<Agent, "status" | "epoch" | "startWait">,
): string | null {
  const wait = agent.startWait;
  if (
    !wait ||
    (agent.status !== AgentStatus.queued &&
      agent.status !== AgentStatus.starting) ||
    wait.epoch !== agent.epoch
  )
    return null;
  switch (wait.reason) {
    case StartWaitReason.disk:
      return "Waiting for free disk space";
    case StartWaitReason.model_catalog:
      return "Waiting for the model catalog";
    case StartWaitReason.provider_session:
      return "Waiting for the provider session";
    case StartWaitReason.dependency:
      return "Waiting for a preparation dependency";
    case StartWaitReason.dispatch_pending:
      return "Waiting for dispatch handoff";
    case StartWaitReason.paused:
      return "Automatic continuation is paused";
    case StartWaitReason.native_failure:
      return "Waiting for native failure recovery";
    case StartWaitReason.account_transfer:
      return "Waiting for account transfer";
    case StartWaitReason.workspace_copy:
      return "Preparing the workspace";
  }
  const unreachable: never = wait;
  return unreachable;
}

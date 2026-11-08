import { AgentRole, AgentStatus } from "../../../generated/api.ts";
import {
  Brain,
  ClipboardList,
  Crown,
  PenLine,
  ScanEye,
  Terminal,
  Timer,
  Wrench,
} from "lucide-react";
import type { Agent } from "../../../types.ts";
import { observer } from "mobx-react-lite";

export const TeamRoleIcon = observer(function TeamRoleIcon({
  agent,
  lead,
}: {
  agent: Agent;
  lead: boolean;
}) {
  const role =
    lead || agent.isLead || ["lead", "orchestrator"].includes(agent.role || "")
      ? "Lead"
      : agent.role === AgentRole.reviewer
        ? "Reviewer"
        : "Worker";
  const Icon = role === "Lead" ? Crown : role === "Reviewer" ? ScanEye : Wrench;
  return (
    <span className="worker-role" role="img" aria-label={role} title={role}>
      <Icon aria-hidden="true" />
    </span>
  );
});

export function activityPresentation(
  agent: Agent,
  detail: Agent | undefined = agent,
) {
  if (detail?.commandProgress?.length)
    return { Icon: Terminal, label: "Command" };
  if (agent.status === AgentStatus.waiting_on_monitor)
    return { Icon: Timer, label: "Waiting on monitor" };
  if (["waiting", "parked", "queued", "starting"].includes(agent.status || ""))
    return { Icon: Timer, label: "Waiting" };
  if (detail?.activity?.phase === "thinking")
    return { Icon: Brain, label: "Thinking" };
  if (detail?.activity?.phase === "writing")
    return { Icon: PenLine, label: "Writing" };
  if (
    detail?.activity?.tools?.some((tool) =>
      /command|exec/i.test(tool.type + tool.name),
    )
  )
    return { Icon: Terminal, label: "Command" };
  if (detail?.activity?.phase === "tool")
    return { Icon: Wrench, label: "Using tools" };
  return { Icon: ClipboardList, label: "Task" };
}

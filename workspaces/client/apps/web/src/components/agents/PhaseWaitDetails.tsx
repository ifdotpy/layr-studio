import { resourceData } from "../../store/runtime.ts";
import { observer } from "mobx-react-lite";
import { useMemo } from "react";
import { useServerStore } from "../../servers/ServerServicesContext.tsx";
import { useActivityView } from "./state.ts";
import { createWaitingChildren } from "./waitDetails.ts";
import type { SidebarAgent } from "../sidebar/viewModel.ts";
import { operationalWait } from "./phasePresentation.ts";

export const PhaseWaitDetails = observer(function PhaseWaitDetails({
  agent,
}: {
  agent: SidebarAgent;
}) {
  const store = useServerStore();
  const children = useMemo(
    () =>
      createWaitingChildren(() => {
        const current = store.state().current;
        return resourceData(current)?.agents.values() ?? [];
      }, agent.id),
    [store, agent],
  );
  const activities = useActivityView(agent);
  const reasons = operationalWait(agent).reasons;
  return (
    <ul>
      {children.agents.map((child) => (
        <li key={child.id}>Agent: {child.name}</li>
      ))}
      {activities.commands.map((command) => (
        <li key={command.id}>Command: {command.command}</li>
      ))}
      {activities.monitors.map((monitor) => (
        <li key={monitor.id}>Monitor: {monitor.command || monitor.id}</li>
      ))}
      {reasons?.event && <li>Event: {reasons.event}</li>}
      {!!reasons?.inputs && (
        <li>
          Input: {reasons.inputs} {reasons.inputs === 1 ? "answer" : "answers"}{" "}
          needed
        </li>
      )}
    </ul>
  );
});

import { resourceData } from "../../../store/runtime.ts";
import { observer } from "mobx-react-lite";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useActivityView } from "../support/state.ts";
import { waitingChildren } from "./waitDetails.ts";
import type { SidebarAgent } from "../../sidebar/support/viewModel.ts";
import { operationalWait } from "../support/phasePresentation.ts";

export const PhaseWaitDetails = observer(function PhaseWaitDetails({
  agent,
}: {
  agent: SidebarAgent;
}) {
  const store = useServerStore();
  const children = waitingChildren(
    resourceData(store.state().current)?.agents.values() ?? [],
    agent.id,
  );
  const activities = useActivityView(agent);
  const reasons = operationalWait(agent).reasons;
  return (
    <ul>
      {children.map((child) => (
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

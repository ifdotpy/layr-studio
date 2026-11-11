import { now } from "../../../clock.ts";
import { busy, type Agent } from "../../../types.ts";
import { useVisualActivity } from "../../../hooks/useVisualActivity.ts";
import { observer } from "mobx-react-lite";

export function progressElapsed(startedAt: number, nowSeconds: number) {
  const seconds = Math.max(0, Math.floor(nowSeconds - startedAt));
  const minutes = Math.floor(seconds / 60);
  return minutes ? `${minutes}m ${seconds % 60}s` : `${seconds}s`;
}

export function commandWaitLabel(agent: Pick<Agent, "commandProgress">) {
  return `Waiting on ${agent.commandProgress?.[0]?.stage || "managed command"}`;
}

/** Visible in agent lists; uses the generated HTTP entity contract. */
export const AgentProgress = observer(function AgentProgress({
  agent,
}: {
  agent: Pick<Agent, "commandProgress" | "stall">;
}) {
  return (
    <>
      {agent.stall && (
        <span
          className="agent-stall"
          role="status"
          title={`The worker finished its turn without submitting task ${agent.stall.taskId}. Its lead decides the next step.`}
        >
          Waiting for lead · task {agent.stall.taskId} not submitted
        </span>
      )}
      {agent.commandProgress?.map((command) => (
        <span
          className="agent-command-progress"
          key={command.monitorId}
          data-monitor={command.monitorId}
        >
          <span className="agent-command-stage" title={command.stage}>
            {command.stage} ·{" "}
            <span aria-label="Elapsed time">
              {progressElapsed(command.startedAt, now() / 1000)}
            </span>
          </span>
          {command.lastOutputLine && (
            <span
              className="agent-command-output"
              title={command.lastOutputLine}
            >
              {command.lastOutputLine}
            </span>
          )}
        </span>
      ))}
    </>
  );
});

/** Age of the current activity; terminal rows freeze at the last server update. */
export const AgentElapsed = observer(function AgentElapsed({
  agent,
  detail = agent,
}: {
  agent: Agent;
  detail?: Agent | undefined;
}) {
  const [visualRef, visible] = useVisualActivity<HTMLSpanElement>();
  const active = busy.has(agent.status || "");
  const started =
    detail.commandProgress?.[0]?.startedAt ||
    detail.activity?.at ||
    agent.created;
  if (started == null) return null;
  return (
    <span
      ref={visualRef}
      className="worker-elapsed"
      aria-label="Elapsed time"
      title="Time since current activity started"
    >
      {progressElapsed(
        started,
        active && visible ? now() / 1000 : agent.updated || Date.now() / 1000,
      )}
    </span>
  );
});

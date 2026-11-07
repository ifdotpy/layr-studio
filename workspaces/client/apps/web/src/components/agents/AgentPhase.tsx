import { observer } from "mobx-react-lite";
import ErrorDescription from "../ErrorDescription";
import type { SidebarAgent } from "../sidebar/viewModel.ts";
import { useAgentDetail, useAgentSummary } from "./state.ts";
import { phasePresentation } from "./phasePresentation.ts";
import { PhaseWaitDetails } from "./PhaseWaitDetails.tsx";
import { useVisualActivityRef } from "../../hooks/useVisualActivity";
import type { MessageConnection } from "../../hooks/messageConnection";
import "./provider-activity.css";

type PresentationProps = {
  connection: MessageConnection | null;
  savedMessagePending?: boolean | undefined;
  connectionDetails?: string | undefined;
  phaseOverride?: string | undefined;
};
const AgentPhase = observer(function AgentPhase({
  agent,
  connection,
  savedMessagePending = false,
  connectionDetails,
  phaseOverride,
}: PresentationProps & { agent: SidebarAgent }) {
  const visualRef = useVisualActivityRef<HTMLDivElement>();
  const detail = useAgentDetail(agent);
  const view = phasePresentation(
    agent,
    detail,
    connection,
    savedMessagePending,
    phaseOverride,
  );
  if (!view) return null;
  const {
    wait,
    notice,
    active,
    phase,
    Icon,
    waiting,
    liveWait,
    nativeMessage,
    nativeErrorMessage,
    message,
    details,
  } = view;
  return (
    <div
      ref={visualRef}
      className={`agent-phase ${(active && !waiting) || liveWait ? "active" : ""}${notice ? " studio-recovery-banner" : ""}`}
      role="status"
      data-phase={phase}
      data-wait-state={waiting ? (wait?.live ? "live" : "ended") : undefined}
      data-agent-provider={agent.provider || ""}
    >
      {/* The phase icon stays in its slot and pulses while the agent works,
          so a changing phase label never moves an activity marker. */}
      {liveWait ? (
        <span className="phase-dots wait-dots" aria-hidden="true">
          <i />
          <i />
          <i />
        </span>
      ) : (
        <Icon size={15} />
      )}
      {liveWait ? (
        <details className="agent-phase-wait">
          <summary>
            <span className="agent-phase-wait-label">{wait.label}</span>
          </summary>
          <div className="agent-phase-wait-details">
            <p>{wait.label}</p>
            <PhaseWaitDetails agent={agent} />
          </div>
        </details>
      ) : (
        <div className="agent-phase-copy">
          <span>
            {notice ? (
              notice.message
            ) : (
              <ErrorDescription
                value={nativeMessage || nativeErrorMessage || message}
                role="status"
              />
            )}
          </span>
          {((savedMessagePending && connectionDetails) ||
            (typeof details === "string" && details)) && (
            <details>
              <summary>Details</summary>
              {savedMessagePending && connectionDetails && (
                <pre>{connectionDetails}</pre>
              )}
              {typeof details === "string" && details && <pre>{details}</pre>}
            </details>
          )}
        </div>
      )}
      {phase === "tool" && (detail?.activity?.tools?.length ?? 0) > 1 && (
        <small>{detail?.activity?.tools?.length} tools</small>
      )}
    </div>
  );
});
export default AgentPhase;

export const LiveAgentPhase = observer(function LiveAgentPhase({
  agentId,
  ...props
}: PresentationProps & { agentId?: string | undefined }) {
  const agent = useAgentSummary(agentId || "");
  return agent ? <AgentPhase agent={agent} {...props} /> : null;
});

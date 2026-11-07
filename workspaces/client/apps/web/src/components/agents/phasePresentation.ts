import { AgentWorktreePreparation } from "../../generated/api";
import { AgentStatus } from "../../generated/api";
import {
  Brain,
  Circle,
  Clock3,
  CircleAlert,
  Pause,
  WifiOff,
  LoaderCircle,
  PenLine,
  Wrench,
} from "lucide-react";
import type { Agent } from "../../types";
import {
  currentCapacityRetry,
  DISK_FULL_WAIT_LABEL,
  diskFullHold,
} from "../../capacityRetry";
import { nativeThreadError } from "../../nativeErrors";
import { startWaitLabel } from "../../startWait";
import { displayError, errorDetails } from "../../errorPresentation";
import { commandWaitLabel } from "./AgentProgress";
import type { MessageConnection } from "../../hooks/messageConnection";

import type { SidebarAgent } from "../sidebar/viewModel.ts";
import {
  formatAgentStatus,
  formatOperationalStatus,
} from "../../status/operationalStatus.ts";

export function operationalWait(agent: SidebarAgent) {
  const status = agent.operationalStatus;
  const reasons =
    status.reason === "waiting" || status.reason === "needs_input"
      ? status.wait
      : null;
  return {
    live: reasons !== null,
    label: formatAgentStatus(agent).label,
    reasons,
  };
}
const endedWaitLabel = formatOperationalStatus({ reason: "turn_ended" }).label;
// Delivery can remain pending while a healthy connection awaits its receipt.
// Choose the copy and icon together so only transport loss looks disconnected.
function phaseNotice(
  connection: MessageConnection | null,
  savedMessagePending: boolean,
) {
  const savedMessage = "Your message is saved and waiting for delivery.";
  if (connection?.message)
    return {
      message: savedMessagePending
        ? `${connection.message} ${savedMessage}`
        : connection.message,
      Icon:
        connection.icon === "connecting"
          ? LoaderCircle
          : connection.icon === "offline"
            ? WifiOff
            : Clock3,
    };
  if (savedMessagePending)
    return {
      message: savedMessage,
      Icon: Clock3,
    };
  return null;
}

export function phasePresentation(
  agent: SidebarAgent,
  detail: Agent | undefined,
  connection: MessageConnection | null,
  savedMessagePending: boolean,
  phaseOverride?: string,
) {
  const wait = operationalWait(agent);
  const notice = phaseNotice(connection, savedMessagePending);
  const safety = detail?.nativeSafetyBuffering;
  if (
    safety?.showBufferingUi === true &&
    safety.dismissed !== true &&
    safety.responseStarted !== true &&
    safety.turnId === agent.turnId &&
    agent.inFlight === true
  )
    return null;
  // The terminal error appears in the transcript and the account notice.
  if (
    agent.status === AgentStatus.failed &&
    detail?.error &&
    !savedMessagePending
  )
    return null;
  const status = agent.status || "idle";
  const active = ["running", "starting"].includes(status);
  const awaitingResponse =
    active &&
    (detail?.startAttempt?.prepareError || detail?.startAttempt?.responseError);
  const currentPhase = nativeThreadError(agent, detail)
    ? "blocked"
    : currentCapacityRetry(agent)?.status === "scheduled"
      ? "capacity-retry"
      : agent.status === AgentStatus.starting && detail?.worktreePreparation
        ? detail?.worktreePreparation === AgentWorktreePreparation["waiting"]
          ? "folder-wait"
          : "folder-prepare"
        : awaitingResponse
          ? "acknowledgement"
          : active
            ? phaseOverride || detail?.activity?.phase || status
            : status;
  const phase =
    wait.live && ["idle", "completed"].includes(currentPhase)
      ? "waiting"
      : currentPhase;
  const names: Record<string, string> = {
    thinking: "Thinking",
    writing: "Writing",
    tool: "Using tools",
    running: "Working",
    starting: startWaitLabel(agent) || "Starting",
    "folder-wait": "Waiting to prepare folder",
    "folder-prepare": "Preparing folder",
    acknowledgement: "Waiting for Codex",
    retrying: "Codex is retrying",
    "capacity-retry": diskFullHold(agent)
      ? DISK_FULL_WAIT_LABEL
      : "Waiting to retry model",
    blocked: "Chat stopped as a precaution",
    auth: "Restoring sign-in",
    safety: "Codex is checking this request",
    error: "Codex reported an error",
    queued: startWaitLabel(agent) || "Queued",
    waiting: wait.label || endedWaitLabel,
    waiting_on_monitor: commandWaitLabel(detail || {}),
    parked: wait.label || endedWaitLabel,
    approval: "Waiting for your answer",
    failed: "Failed",
    paused: "Stopped",
    interrupted: "Interrupted",
    idle: "Ready",
  };
  if (["idle", "completed"].includes(phase) && !notice) return null;
  const icons: Record<string, typeof Brain> = {
    thinking: Brain,
    writing: PenLine,
    tool: Wrench,
    starting: LoaderCircle,
    "folder-wait": Clock3,
    "folder-prepare": LoaderCircle,
    running: LoaderCircle,
    queued: Clock3,
    waiting: Circle,
    waiting_on_monitor: Clock3,
    acknowledgement: Clock3,
    retrying: LoaderCircle,
    "capacity-retry": Clock3,
    auth: Clock3,
    safety: Clock3,
    error: CircleAlert,
    blocked: CircleAlert,
    approval: Clock3,
    failed: CircleAlert,
    interrupted: CircleAlert,
    paused: Pause,
  };
  const Icon = notice?.Icon || icons[phase] || Circle;
  const waiting = ["waiting", "parked"].includes(phase);
  const liveWait = waiting && wait.live && !notice;
  const native = ["retrying", "auth", "safety"].includes(phase)
    ? typeof detail?.nativeStatus === "object"
      ? detail?.nativeStatus
      : null
    : null;
  const nativeError = typeof native?.error === "object" ? native.error : null;
  const nativeMessage = native?.message || undefined;
  const nativeErrorMessage = nativeError?.message;
  const message =
    displayError(nativeMessage) ||
    displayError(nativeErrorMessage) ||
    names[phase] ||
    "Working";
  const details = errorDetails(nativeError?.additionalDetails);

  return {
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
  };
}

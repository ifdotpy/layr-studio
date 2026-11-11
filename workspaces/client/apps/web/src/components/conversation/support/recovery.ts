import type { PostResult } from "../../../api.ts";
import { DISK_FULL_CAUSE, type CapacityRetry } from "../../../capacityRetry.ts";
import type { Agent } from "../../../types.ts";
import type { components } from "../../../generated/api.ts";
import { localDateTime } from "../../../local-time.ts";
import { now } from "../../../clock.ts";
type LimitRecovery = components["schemas"]["LimitRecovery"];

const capacityStatus: Partial<Record<string, string>> = {
  cancelled: "Automatic retry cancelled.",
  exhausted: "Automatic retries finished. You can retry now.",
  unknown: "Waiting for Codex to confirm the retry.",
  failed: "The retry could not start.",
};

export function capacityProjection(
  retry: CapacityRetry,
  confirmed: PostResult<"/api/capacity-retry"> | undefined,
) {
  const current =
    confirmed &&
    confirmed.id === retry.id &&
    (confirmed.updatedAt || 0) > (retry.updatedAt || 0)
      ? confirmed
      : retry;
  const scheduled = current.status === "scheduled";
  const seconds =
    scheduled && current.dueAt
      ? Math.max(0, Math.ceil(current.dueAt - now() / 1000))
      : 0;
  const message = scheduled
    ? current.cause === DISK_FULL_CAUSE
      ? "Waiting for free disk space."
      : seconds > 0
        ? `Automatic retry in ${seconds}s.`
        : "Waiting for the server to retry."
    : (capacityStatus[current.status || ""] ??
      (current.acceptedTurnId
        ? "The model retry started."
        : "Starting the model retry."));
  return {
    current,
    scheduled,
    message,
    available:
      !current.claimedAt &&
      ["scheduled", "cancelled", "exhausted"].includes(current.status),
  };
}

export function resumeProjection(
  usageResume: Agent["usageResume"] | undefined,
  confirmed: PostResult<"/api/usage-resume"> | undefined,
  agentId?: string,
) {
  const resume =
    confirmed && (confirmed.updatedAt || 0) > (usageResume?.updatedAt || 0)
      ? confirmed
      : usageResume;
  const scheduled = resume?.status === "scheduled";
  return {
    resume,
    scheduled,
    resumeAt: resume?.plannedAt ?? resume?.dueAt,
    auth: resume?.cause === "auth",
    canToggle:
      !!agentId &&
      !!resume?.id &&
      (scheduled ||
        resume?.reason === "Automatic resume is off for this chat."),
  };
}

const limitStatus: Partial<Record<string, string>> = {
  checking: "The reset time passed. Studio is checking the account.",
  confirmed: "The limit has reset.",
  unconfirmed:
    "The reset could not be confirmed. Refresh to check the account.",
};

export function limitLine(timing: LimitRecovery["timing"] | undefined) {
  if (timing?.kind === "waiting")
    return `Limit reached. Resets at ${localDateTime(new Date(timing.resetAt * 1000))}.`;
  return limitStatus[timing?.kind || ""] ?? "Limit reached.";
}

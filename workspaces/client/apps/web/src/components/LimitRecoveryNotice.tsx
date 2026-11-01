import { useServerApi } from "../servers/ServerServicesContext";
import { localDateTime } from "../local-time";
import { useState } from "react";
import { Button } from "@mantine/core";
import type { components } from "../generated/api";
type LimitRecovery = components["schemas"]["LimitRecovery"];
import { copyText } from "../clipboard/clipboard";
import { errorText, type PostResult } from "../api";
import type { Agent } from "../types";

function limitLine(timing: LimitRecovery["timing"] | undefined) {
  switch (timing?.kind) {
    case "waiting":
      return `Limit reached. Resets at ${localDateTime(new Date(timing.resetAt * 1000))}.`;
    case "checking":
      return "The reset time passed. Studio is checking the account.";
    case "confirmed":
      return "The limit has reset.";
    case "unconfirmed":
      return "The reset could not be confirmed. Refresh to check the account.";
    default:
      return "Limit reached.";
  }
}

export default function LimitRecoveryNotice({
  recovery,
  agentId,
  usageResume,
  inline = false,
  tabIndex,
  onRefresh,
}: {
  recovery?: LimitRecovery | undefined;
  agentId?: string;
  usageResume?: Agent["usageResume"] | undefined;
  inline?: boolean;
  tabIndex?: number | undefined;
  /** Read the account limits again. */
  onRefresh?: (() => Promise<void> | void) | undefined;
}) {
  const { post } = useServerApi();
  const [refreshing, setRefreshing] = useState(false);

  const [copyStatus, setCopyStatus] = useState("");
  const [confirmed, setConfirmed] =
    useState<PostResult<"/api/usage-resume"> | null>(null);
  const [busy, setBusy] = useState(false);
  const [resumeError, setResumeError] = useState("");
  const resume =
    confirmed && (confirmed.updatedAt || 0) > (usageResume?.updatedAt || 0)
      ? confirmed
      : usageResume;
  const scheduled = resume?.status === "scheduled";
  const plannedAt = resume?.plannedAt;
  const nextCheckAt = resume?.dueAt;
  const auth = resume?.cause === "auth";
  const canToggle =
    !!agentId &&
    !!resume?.id &&
    (scheduled || resume?.reason === "Automatic resume is off for this chat.");
  const toggleResume = async (enabled: boolean) => {
    if (!agentId || !resume?.id || busy) return;
    setBusy(true);
    setResumeError("");
    try {
      const updated = await post("/api/usage-resume", {
        id: agentId,
        resume_id: resume.id,
        enabled,
      });
      setConfirmed(updated);
    } catch (error) {
      setResumeError(
        `Automatic resume choice could not be confirmed. ${errorText(error)}`,
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <div
      className={`account-limit-recovery${inline ? " inline" : ""}`}
      role="status"
    >
      {auth ? (
        <>
          <strong>
            {scheduled
              ? "Waiting for the account sign-in"
              : "Automatic resume is off"}
          </strong>
          <p>
            {scheduled
              ? "Studio will continue this chat when the account works again."
              : "Turn on automatic resume to continue this chat after sign-in works."}
          </p>
          {scheduled &&
            (typeof plannedAt === "number" ||
              typeof nextCheckAt === "number") && (
              <p>
                {typeof plannedAt === "number"
                  ? "Automatic resume planned for "
                  : "Next account check "}
                <time
                  dateTime={new Date(
                    (plannedAt ?? nextCheckAt!) * 1000,
                  ).toISOString()}
                >
                  {localDateTime(new Date((plannedAt ?? nextCheckAt!) * 1000))}
                </time>
              </p>
            )}
        </>
      ) : (
        <p>
          {limitLine(recovery?.timing)}
          {!scheduled && onRefresh && (
            <>
              {" "}
              <Button
                size="compact-xs"
                variant="light"
                loading={refreshing}
                tabIndex={tabIndex}
                onClick={async () => {
                  setRefreshing(true);
                  try {
                    await onRefresh();
                  } finally {
                    setRefreshing(false);
                  }
                }}
              >
                Refresh
              </Button>
            </>
          )}
        </p>
      )}
      {resume?.reason === "Automatic resume is off for this chat." && (
        <p>Automatic resume is off for this chat.</p>
      )}
      {canToggle && (
        <Button
          tabIndex={tabIndex}
          size="compact-xs"
          disabled={busy}
          onClick={() => void toggleResume(!scheduled)}
        >
          {scheduled ? "Turn off automatic resume" : "Turn on automatic resume"}
        </Button>
      )}
      {resumeError && <p role="alert">{resumeError}</p>}
      {recovery?.action.kind === "link" && (
        <>
          <Button
            tabIndex={tabIndex}
            component="a"
            size="compact-xs"
            href={recovery.action.href}
            target="_blank"
            rel="noreferrer"
          >
            {recovery.action.label}
          </Button>
          <small>Use this chat's account in ChatGPT.</small>
        </>
      )}
      {recovery?.action.kind === "ask_owner" && (
        <>
          <Button
            tabIndex={tabIndex}
            size="compact-xs"
            onClick={async () => {
              try {
                await copyText(
                  recovery.action.kind === "ask_owner"
                    ? recovery.action.request
                    : "",
                );
                setCopyStatus(
                  "Request copied. Send it to your workspace owner.",
                );
              } catch {
                setCopyStatus(
                  recovery.action.kind === "ask_owner"
                    ? recovery.action.request
                    : "",
                );
              }
            }}
          >
            Copy request for owner
          </Button>
          {copyStatus && <p>{copyStatus}</p>}
        </>
      )}
    </div>
  );
}

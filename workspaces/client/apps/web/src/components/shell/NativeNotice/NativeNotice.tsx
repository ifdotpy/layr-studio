import type { AgentDetailEntityDto } from "../../../generated/observableResources.ts";
import { AgentStatus as GeneratedApiAgentStatus } from "../../../generated/api.ts";
import { displayError, errorDetails } from "../../../errorPresentation.ts";
import { currentCapacityRetry } from "../../../capacityRetry.ts";
import CapacityRetry from "../../conversation/CapacityRetry/CapacityRetry.tsx";
import LimitRecoveryNotice from "../../conversation/LimitRecoveryNotice/LimitRecoveryNotice.tsx";
import ConnectionRecovery from "../ConnectionRecovery/ConnectionRecovery.tsx";
import type { Agent, Message } from "../../../types.ts";
import type { components } from "../../../generated/api.ts";
import { agentErrorLabel } from "../../../types.ts";
import { Button } from "@mantine/core";
import { CircleAlert } from "lucide-react";
import { failureMessage } from "../../conversation/support/turnFailureReason.ts";
import { nativeErrorView, nativeThreadError } from "../../../nativeErrors.ts";

function Guidance({
  error,
  cyberAccessProgram,
}: {
  error: ReturnType<typeof nativeErrorView>;
  cyberAccessProgram?: string | null | undefined;
}) {
  return (
    <>
      {error.kind === "cyberPolicy" && <small>{error.message}</small>}
      {error.hint && <small>{error.hint}</small>}
      {error.kind === "cyberPolicy" && cyberAccessProgram && (
        <small>Cyber access program: {cyberAccessProgram}</small>
      )}
      {!!error.links.length && (
        <div className="native-error-links">
          {error.links.map((link) => (
            <a
              key={link.href}
              href={link.href}
              target="_blank"
              rel="noreferrer"
            >
              {link.label}
            </a>
          ))}
        </div>
      )}
    </>
  );
}

export { isNonBlockingWarning } from "../../../app/notices/nonBlockingWarning.ts";

export function NativeError({
  agent,
  detail = agent,
  planType,
  recovery,
  openLimits,
  refreshLimits,
  newChat,
  chooseChat,
}: {
  agent: Agent;
  detail?: AgentDetailEntityDto | Agent | undefined;
  planType?: string | undefined;
  recovery?: components["schemas"]["LimitRecovery"] | null | undefined;
  openLimits?: () => void;
  /** Read the account limits of this chat again. */
  refreshLimits?: () => void;
  newChat?: (() => void) | undefined;
  chooseChat?: (() => void) | undefined;
}) {
  const usageResume = detail.usageResume;
  const authResume =
    usageResume?.cause === "auth" &&
    (usageResume.status === "scheduled" ||
      usageResume.reason === "Automatic resume is off for this chat.");
  const retry = currentCapacityRetry(agent);
  const blocked = nativeThreadError(agent, detail);
  const connectionCheck = detail.manualConnectionCheck?.receipt;
  const error = nativeErrorView(blocked || detail.error, planType);
  const detailView = connectionCheck
    ? nativeErrorView(detail.error, planType)
    : error;
  const wait = detail.contextRepairWait;
  if (
    !blocked &&
    !retry &&
    !connectionCheck &&
    agent.status === GeneratedApiAgentStatus.queued &&
    wait?.scope === "local" &&
    typeof detail.error === "string" &&
    detail.error === wait.error &&
    detail.error.startsWith("Context repair waits for monitors: ")
  )
    return null;
  return (
    <div
      className={`native-error ${recovery ? "usage-limit" : retry ? "warning" : error.severity}`}
      role="alert"
    >
      {!recovery && !authResume && (
        <span>
          {(connectionCheck && "Previous turn needs review") ||
            error.title ||
            (blocked ? error.message : agentErrorLabel(agent, detail.error)) ||
            (retry ? "Model retry" : "")}
        </span>
      )}
      {connectionCheck ? (
        <small>
          {connectionCheck.nativeState === "active"
            ? "Codex reported this thread as active. Its outcome is not confirmed."
            : "Codex answered the connection check. The previous outcome is unconfirmed. Review the history before continuing."}
        </small>
      ) : !recovery && !authResume && !retry ? (
        <Guidance
          error={error}
          cyberAccessProgram={detail.cyberAccessProgram}
        />
      ) : null}
      {(recovery || authResume) && (
        <LimitRecoveryNotice
          key={JSON.stringify([recovery, usageResume?.id])}
          recovery={recovery ?? undefined}
          agentId={agent.id}
          usageResume={usageResume ?? undefined}
          inline
          onRefresh={refreshLimits}
        />
      )}
      {retry && (
        <CapacityRetry
          key={String(retry.id)}
          agentId={agent.id}
          retry={retry}
        />
      )}
      {detail.manualConnectionCheck?.available && (
        <ConnectionRecovery
          key={JSON.stringify([
            agent.id,
            agent.threadId,
            agent.turnId,
            agent.epoch,
            agent.accountKey,
            detail.error,
          ])}
          agentId={agent.id}
        />
      )}
      {!!blocked && (
        <div className="native-error-actions">
          {newChat && (
            <Button size="compact-xs" onClick={newChat}>
              New chat
            </Button>
          )}
          {chooseChat && (
            <Button size="compact-xs" onClick={chooseChat}>
              Choose another chat
            </Button>
          )}
        </div>
      )}
      {openLimits &&
        ["usageLimitExceeded", "rateLimitExceeded"].includes(error.kind) && (
          <Button
            size="xs"
            variant="light"
            {...(recovery ? { color: "gray" } : {})}
            onClick={openLimits}
          >
            View account limits
          </Button>
        )}
      {(connectionCheck || error.details || (recovery && error.message)) && (
        <details>
          <summary>Details</summary>
          <pre>{detailView.details || detailView.message}</pre>
        </details>
      )}
    </div>
  );
}

export function NativeNotice({
  item,
  planType,
}: {
  item: Message;
  planType?: string | undefined;
}) {
  if ("nativeHookQuiet" in item && item.nativeHookQuiet) return null;
  const nativeErrorValue = "nativeError" in item ? item.nativeError : undefined;
  const notice: "error" | "warning" | undefined =
    "nativeNotice" in item &&
    (item.nativeNotice === "error" || item.nativeNotice === "warning")
      ? item.nativeNotice
      : undefined;
  const error = nativeErrorValue
    ? nativeErrorView(nativeErrorValue, planType)
    : null;
  const details =
    error?.details || ("details" in item ? item.details : undefined);
  const cyberAccessProgram =
    "cyberAccessProgram" in item && typeof item.cyberAccessProgram === "string"
      ? item.cyberAccessProgram
      : undefined;
  if (
    error &&
    ["usageLimitExceeded", "rateLimitExceeded"].includes(error.kind)
  ) {
    const raw = details || error.message;
    return (
      <div
        className="native-notice native-limit-notice"
        data-message={item.id}
        data-source-message={item.sourceId}
      >
        <div className="native-limit-notice-heading">
          <CircleAlert size={16} aria-hidden="true" />
          <strong>{error.title}</strong>
        </div>
        <p>{error.hint}</p>
        {raw && (
          <details>
            <summary>Details</summary>
            <pre>
              {typeof raw === "string" ? raw : JSON.stringify(raw, null, 2)}
            </pre>
          </details>
        )}
      </div>
    );
  }
  return (
    <div
      className={`native-notice ${error?.severity || notice}`}
      data-message={item.id}
      data-source-message={item.sourceId}
    >
      <details>
        <summary>
          {notice === "error"
            ? failureMessage(nativeErrorValue || item.text)
            : error?.title || error?.message || displayError(item.text)}
        </summary>
        {Boolean(details) && <pre>{errorDetails(details)}</pre>}
      </details>
      {error && (
        <Guidance error={error} cyberAccessProgram={cyberAccessProgram} />
      )}
    </div>
  );
}

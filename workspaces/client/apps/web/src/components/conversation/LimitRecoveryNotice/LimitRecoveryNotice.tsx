import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { localDateTime } from "../../../local-time.ts";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { CopyFeedback } from "../../../stores/CopyFeedback.ts";
import { AsyncFeedback } from "../../../stores/AsyncFeedback.ts";
import { observable } from "mobx";
import { limitLine, resumeProjection } from "../support/recovery.ts";
import { Button } from "@mantine/core";
import type { components } from "../../../generated/api.ts";
type LimitRecovery = components["schemas"]["LimitRecovery"];
import type { PostResult } from "../../../api.ts";
import type { Agent } from "../../../types.ts";

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
  return (
    <Owned
      create={() => ({
        resume: new AsyncFeedback(),
        confirmed: observable.box<PostResult<"/api/usage-resume">>(),
        refresh: new AsyncFeedback(),
        copy: new CopyFeedback(),
      })}
    >
      {(state) => {
        const { resume, scheduled, resumeAt, auth, canToggle } =
          resumeProjection(usageResume, state.confirmed.get(), agentId);
        const toggleResume = (enabled: boolean) => {
          if (agentId && resume?.id)
            void state.resume.run(
              () =>
                post("/api/usage-resume", {
                  id: agentId,
                  resume_id: resume.id,
                  enabled,
                }),
              (result) => state.confirmed.set(result),
            );
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
                      loading={state.refresh.pending}
                      tabIndex={tabIndex}
                      onClick={() =>
                        void state.refresh.run(() =>
                          Promise.resolve(onRefresh()),
                        )
                      }
                    >
                      Refresh
                    </Button>
                  </>
                )}
              </p>
            )}
            {scheduled && typeof resumeAt === "number" && (
              <p>
                {typeof resume?.plannedAt === "number"
                  ? "Automatic resume planned for "
                  : "Next account check "}
                <time dateTime={new Date(resumeAt * 1000).toISOString()}>
                  {localDateTime(new Date(resumeAt * 1000))}
                </time>
              </p>
            )}
            {resume?.reason === "Automatic resume is off for this chat." && (
              <p>Automatic resume is off for this chat.</p>
            )}
            {canToggle && (
              <Button
                tabIndex={tabIndex}
                size="compact-xs"
                disabled={state.resume.pending}
                onClick={() => void toggleResume(!scheduled)}
              >
                {scheduled
                  ? "Turn off automatic resume"
                  : "Turn on automatic resume"}
              </Button>
            )}
            {state.resume.error && (
              <p role="alert">
                Automatic resume choice could not be confirmed.{" "}
                {state.resume.error}
              </p>
            )}
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
                  onClick={() =>
                    void state.copy.copy(
                      recovery.action.kind === "ask_owner"
                        ? recovery.action.request
                        : "",
                    )
                  }
                >
                  Copy request for owner
                </Button>
                {state.copy.copied() && (
                  <p>Request copied. Send it to your workspace owner.</p>
                )}
                {state.copy.failed && <p>{recovery.action.request}</p>}
              </>
            )}
          </div>
        );
      }}
    </Owned>
  );
}

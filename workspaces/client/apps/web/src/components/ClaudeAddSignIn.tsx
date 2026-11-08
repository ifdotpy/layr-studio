import { observer } from "mobx-react-lite";
import { requestId as resourceRequestId } from "../generated/observableResources.ts";
import { resourceData } from "../store/runtime.ts";
import {
  useServerStore,
  useServerResources,
} from "../servers/ServerServicesContext";
import { randomUUID } from "../uuid";
import { useServerApi } from "../servers/ServerServicesContext";
import { Button, Group, Modal, Stack, TextInput } from "@mantine/core";
import { useEffect, useRef, useState } from "react";
import { errorText, type PostResult } from "../api";
import { copyText } from "../clipboard/clipboard";
import { ClaudeAuthorizationCodeForm } from "./ClaudeAuthorizationCodeForm";
import {
  codeSubmissionNotApplied,
  codeSubmissionResolved,
  codeVerificationPending,
} from "./claudeCodeSubmission";

type Receipt = PostResult<"/api/accounts/claude/login">;
type SavedFlow = {
  requestId: string;
  startRequestId: string;
  codeRequestId?: string | undefined;
  cancelRequestId?: string;
  label: string;
  email: string;
  createdAt: number;
  unknownSince?: number | undefined;
  codeSubmissionUncertain?: boolean;
};
const UNKNOWN_REQUEST_GRACE_MS = 5000;
const terminal = (status?: string) =>
  ["ready", "error", "cancelled"].includes(status || "");
const attemptKey = (key: string, id: string) => `${key}:${id}`;

function readSavedFlow(
  key: string,
  { save, saved }: Pick<ReturnType<typeof useServerApi>, "save" | "saved">,
): SavedFlow | null {
  const normalize = (flow: SavedFlow): SavedFlow => ({
    ...flow,
    createdAt: flow.createdAt || Date.now(),
    codeSubmissionUncertain:
      flow.codeSubmissionUncertain ?? !!flow.codeRequestId,
  });
  const active = saved<unknown>(key, null);
  if (typeof active === "string") {
    const flow = saved<SavedFlow | null>(attemptKey(key, active), null);
    if (!flow) save(key, null);
    return flow ? normalize(flow) : null;
  }
  if (
    active &&
    typeof active === "object" &&
    "requestId" in active &&
    typeof active.requestId === "string"
  ) {
    const legacy = normalize(active as SavedFlow);
    save(attemptKey(key, legacy.requestId), {
      ...legacy,
    });
    save(key, legacy.requestId);
    return legacy;
  }
  return null;
}

function isUnknownRequest(failure: unknown) {
  const status =
    failure && typeof failure === "object" && "status" in failure
      ? failure.status
      : undefined;
  return (
    status === 404 || /unknown claude sign-in request/i.test(errorText(failure))
  );
}

function allowedUrl(value?: string | null) {
  try {
    const url = new URL(value || "");
    return url.protocol === "https:" &&
      !url.username &&
      !url.password &&
      !url.port &&
      url.hostname === "claude.com" &&
      url.pathname === "/cai/oauth/authorize"
      ? url.href
      : null;
  } catch {
    return null;
  }
}

export default observer(function ClaudeAddSignIn({
  label,
  email,
  serverLabel,
  scope,
  opened,
  onClose,
}: {
  label: string;
  email: string;
  serverLabel: string;
  scope: string;
  opened: boolean;
  onClose: () => void;
}) {
  const { post, save, saved } = useServerApi();

  const storageKey = `claude-add-sign-in:${scope}`;
  const [flow, setFlow] = useState<SavedFlow | null>(() =>
    readSavedFlow(storageKey, { save, saved }),
  );
  const requestId = flow?.requestId || "";
  const storeView = useServerStore();
  const resources = useServerResources();
  const observed = requestId
    ? storeView.authProgress(resourceRequestId(requestId)).current
    : undefined;
  const observedReceipt = observed ? resourceData(observed) : undefined;
  const receipt =
    observedReceipt?.requestId === requestId &&
    observedReceipt.status !== "duplicate" &&
    observedReceipt.status !== "uncertain"
      ? (observedReceipt as Receipt)
      : null;
  const publishReceipt = (result: Receipt) => {
    resources.publish(
      { kind: "auth-progress", requestId: result.requestId },
      result,
    );
  };
  const [expectedEmail, setExpectedEmail] = useState(email);
  const [code, setCode] = useState("");
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [now, setNow] = useState(() => Date.now());
  const lock = useRef(false);
  const persistFlow = (next: SavedFlow) => {
    save(attemptKey(storageKey, next.requestId), next);
    save(storageKey, next.requestId);
    setFlow(next);
  };
  const clearSavedFlow = (savedFlow: SavedFlow) => {
    save(attemptKey(storageKey, savedFlow.requestId), null);
    if (saved<string>(storageKey, "") === savedFlow.requestId)
      save(storageKey, null);
  };
  const start = async (emailHint = expectedEmail, forceNew = false) => {
    if (lock.current) return;
    lock.current = true;
    setBusy("start");
    setError("");
    const canResume =
      !forceNew &&
      flow &&
      (!receipt || ["starting", "pending"].includes(receipt.status));
    const next: SavedFlow = canResume
      ? { ...flow, unknownSince: undefined }
      : {
          requestId: randomUUID(),
          startRequestId: randomUUID(),
          label: label.trim() || "Claude Code",
          email: emailHint.trim(),
          createdAt: Date.now(),
        };
    if (!canResume && flow) clearSavedFlow(flow);
    persistFlow(next);
    try {
      const result = await post(
        "/api/accounts/claude/add",
        {
          login_id: next.requestId,
          label: next.label,
          ...(next.email ? { email: next.email } : {}),
        },
        { requestId: next.startRequestId },
      );
      publishReceipt(result);
      setExpectedEmail(next.email);
    } catch (failure) {
      setError(errorText(failure));
    } finally {
      lock.current = false;
      setBusy("");
    }
  };
  const run = async (action: "code" | "cancel") => {
    if (lock.current || !requestId || !flow) return;
    lock.current = true;
    setBusy(action);
    setError("");
    const submitted = code.trim();
    const requestField =
      action === "code" ? "codeRequestId" : "cancelRequestId";
    const mutationId = flow[requestField] || randomUUID();
    const nextFlow: SavedFlow = {
      ...flow,
      [requestField]: mutationId,
      ...(action === "code" ? { codeSubmissionUncertain: true } : {}),
    };
    persistFlow(nextFlow);
    try {
      const result = await post(
        action === "code"
          ? "/api/accounts/claude/login/code"
          : "/api/accounts/claude/login/cancel",
        action === "code"
          ? { login_id: requestId, code: submitted }
          : { login_id: requestId },
        { requestId: mutationId },
      );
      publishReceipt(result);
      if (action === "code") {
        setCode("");
        const confirmedSubmitted = codeSubmissionResolved(result);
        persistFlow({
          ...nextFlow,
          codeRequestId: confirmedSubmitted ? undefined : mutationId,
          codeSubmissionUncertain: !confirmedSubmitted,
        });
      } else {
        persistFlow({ ...nextFlow, [requestField]: undefined });
      }
    } catch (failure) {
      if (action === "code" && codeSubmissionNotApplied(failure)) {
        persistFlow({ ...nextFlow, codeSubmissionUncertain: false });
        setError(errorText(failure));
      } else if (action === "code") {
        persistFlow({ ...nextFlow, codeSubmissionUncertain: true });
        setCode("");
      } else if (action === "cancel" && isUnknownRequest(failure)) {
        clearSavedFlow(flow);
        setFlow(null);
        setError("");
      } else setError(errorText(failure));
    } finally {
      lock.current = false;
      setBusy("");
    }
  };
  useEffect(() => {
    if (observed?.status !== "error") return;
    if (isUnknownRequest(observed.error) && flow && !flow.unknownSince) {
      persistFlow({ ...flow, unknownSince: Date.now() });
      setError("");
    } else if (!isUnknownRequest(observed.error))
      setError(errorText(observed.error));
  }, [observed]);
  useEffect(() => {
    if (
      receipt &&
      flow &&
      codeSubmissionResolved(receipt) &&
      flow.codeSubmissionUncertain
    )
      persistFlow({
        ...flow,
        codeSubmissionUncertain: false,
        codeRequestId: receipt.codeSubmitted ? undefined : flow.codeRequestId,
      });
  }, [receipt]);
  useEffect(() => {
    if (!flow?.unknownSince || !requestId || receipt) return;
    const remaining =
      UNKNOWN_REQUEST_GRACE_MS - (Date.now() - flow.unknownSince);
    if (remaining <= 0) {
      setNow(Date.now());
      return;
    }
    const timer = setTimeout(() => setNow(Date.now()), remaining);
    return () => clearTimeout(timer);
  }, [flow?.unknownSince, receipt, requestId]);
  useEffect(() => {
    if (flow && terminal(receipt?.status)) clearSavedFlow(flow);
  }, [flow?.requestId, receipt?.status]);
  const unknownTooLong =
    !!flow?.unknownSince &&
    !receipt &&
    now - flow.unknownSince >= UNKNOWN_REQUEST_GRACE_MS;
  const codeSubmissionUncertain = flow?.codeSubmissionUncertain === true;
  const close = () => {
    if (flow && terminal(receipt?.status)) clearSavedFlow(flow);
    onClose();
  };
  const url = allowedUrl(receipt?.verificationUrl);
  const displayLabel = flow?.label || label;
  const displayEmail = flow?.email || expectedEmail || email;
  const wrong = !!receipt?.error?.includes("different Claude account");
  const expired = !!receipt?.error?.includes("expired");
  const shownError = wrong
    ? `You signed in as ${receipt?.email || "another account"}. This account expects ${email || "a different account"}. Studio did not save the sign-in.`
    : expired
      ? "The sign-in expired. Start a new sign-in to get a new link."
      : error || receipt?.error || "";
  return (
    <Modal
      opened={opened}
      onClose={close}
      title={`Sign in to Claude · ${displayLabel}`}
      centered
    >
      <Stack gap="sm">
        <TextInput label="Server" value={serverLabel} readOnly />
        {displayEmail && <p>Sign in as {displayEmail}.</p>}
        {receipt?.status === "ready" ? (
          <p role="status">
            Signed in as {receipt.email || email || "Claude account"}.
          </p>
        ) : (
          <>
            {receipt?.status === "cancelled" && (
              <p role="status">Sign-in cancelled.</p>
            )}
            {url && (
              <Group>
                <Button
                  component="a"
                  href={url}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  Open sign-in page
                </Button>
                <Button
                  variant="default"
                  disabled={!!busy}
                  onClick={() =>
                    void (async () => {
                      try {
                        await copyText(url);
                        setCopied(true);
                      } catch (failure) {
                        setError(errorText(failure));
                      }
                    })()
                  }
                >
                  {copied ? "Link copied" : "Copy link"}
                </Button>
              </Group>
            )}
            {receipt?.verificationUrl && !url && (
              <p role="alert">Claude returned an unsupported sign-in link.</p>
            )}
            {codeSubmissionUncertain && (
              <p role="status">
                Studio could not confirm whether Claude received the
                authorization response. Check sign-in status or cancel this
                request; do not submit it again.
              </p>
            )}
            {receipt?.status === "pending" &&
              (codeVerificationPending(receipt) ? (
                <p role="status">
                  The full authorization response was submitted. Waiting for
                  Claude to verify it; do not submit it again.
                </p>
              ) : !codeSubmissionUncertain ? (
                <ClaudeAuthorizationCodeForm
                  value={code}
                  onChange={setCode}
                  onSubmit={() => void run("code")}
                  disabled={!!busy}
                  loading={busy === "code"}
                  submitLabel="Submit authorization response"
                />
              ) : null)}
            {(receipt?.status === "pending" || codeSubmissionUncertain) && (
              <Button
                variant="default"
                disabled={!!busy}
                onClick={() => resources.retryNow()}
              >
                Check sign-in status
              </Button>
            )}
            {!requestId || terminal(receipt?.status) ? (
              <Button
                onClick={() => void start(displayEmail)}
                loading={busy === "start"}
                disabled={!!busy}
              >
                {receipt ? "Start again" : "Start sign-in"}
              </Button>
            ) : (
              !receipt &&
              !codeSubmissionUncertain && (
                <Group>
                  <Button
                    onClick={() => void start(displayEmail)}
                    loading={busy === "start"}
                    disabled={!!busy}
                  >
                    Retry sign-in
                  </Button>
                  {unknownTooLong && !codeSubmissionUncertain && (
                    <Button
                      variant="default"
                      onClick={() => void start(displayEmail, true)}
                      disabled={!!busy}
                    >
                      Start again
                    </Button>
                  )}
                </Group>
              )
            )}
            {requestId &&
              receipt &&
              ["starting", "pending"].includes(receipt.status) && (
                <p role="status">Waiting for sign-in on {serverLabel}…</p>
              )}
            {requestId &&
              (["starting", "pending"].includes(receipt?.status || "") ||
                !receipt) && (
                <Button
                  variant="subtle"
                  onClick={() => void run("cancel")}
                  disabled={!!busy}
                  loading={busy === "cancel"}
                >
                  Cancel
                </Button>
              )}
            {unknownTooLong && !codeSubmissionUncertain && (
              <p role="status">
                The server has not found this request. Retry it, start again, or
                cancel.
              </p>
            )}
            {wrong && receipt?.email && (
              <Button
                variant="default"
                onClick={() => {
                  setExpectedEmail(receipt.email!);
                  void start(receipt.email!, true);
                }}
                disabled={!!busy}
              >
                Keep {receipt.email}
              </Button>
            )}
          </>
        )}
        {shownError && <p role="alert">{shownError}</p>}
        <Group justify="flex-end">
          <Button variant="subtle" onClick={close}>
            Close
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
});

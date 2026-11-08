import { useMessageRenderProbe } from "./renderProbe";
import { useUnsentText } from "../../../unsent/useUnsentText";
import {
  complaintId,
  type ComplaintDetailResponse,
  type StateCollections,
} from "../../../generated/observableResources";
import { resourceData } from "../../../store/runtime";
import { useConversationActions } from "../../conversation/navigation";
import {
  useServerApi,
  useServerStorage,
  useServerStore,
} from "../../../servers/ServerServicesContext";
import { Button, Textarea } from "@mantine/core";
import { useEffect, useMemo, useState } from "react";
import { autorun, observable, runInAction } from "mobx";
import { observer } from "mobx-react-lite";
import { ApiError, errorText } from "../../../api";
import "./complaint-book.css";
import ErrorDescription from "../../ErrorDescription";
import MessageDate from "../../conversation/transcript/MessageDate";
import StreamingText from "../../conversation/transcript/StreamingText";
import AgentAvatar from "../../agents/AgentAvatar";

import {
  complaintReplyRequest,
  complaintResponseStore,
  isDefinitiveComplaintRefusal,
  submitComplaintResponse,
  type UserComplaintResponse,
} from "./complaintReplyRequest";

// Preserve confirmed replies while a cached snapshot catches up.
type ComplaintSummary =
  StateCollections["complaints"] extends ReadonlyMap<string, infer T>
    ? T
    : never;
const caches = new WeakMap<Storage, Map<string, ComplaintDetailResponse>>();
function useMessageCache() {
  const storage = useServerStorage();
  let cache = caches.get(storage);
  if (!cache) {
    cache = observable.map(undefined, { deep: false });
    caches.set(storage, cache);
  }
  return cache;
}

function MessageResponse({
  detail,
  requests,
  scope,
  onResponse,
}: {
  detail: ComplaintDetailResponse;
  requests: Map<string, UserComplaintResponse>;
  scope: string;
  onResponse: (c: ComplaintDetailResponse) => void;
}) {
  const { get, post } = useServerApi();
  const area = useConversationActions();
  const { refresh } = area;
  const { notify } = area.notices;
  const storage = useServerStorage();
  const responseStore = complaintResponseStore(storage, scope);

  const draftKey = JSON.stringify([
    `studio-message-responses:${scope}`,
    detail.id,
  ]);
  const [savedReply, saveReply, clearReply] = useUnsentText("reply", draftKey);
  const previous = requests.get(detail.id);
  const [text, setText] = useState(previous?.text || "");
  useEffect(() => {
    if (!previous && savedReply !== null) setText(savedReply);
  }, [savedReply]);
  const status: UserComplaintResponse["status"] =
    previous?.status || "in_progress";
  const [sending, setSending] = useState(false);
  const [retry, setRetry] = useState(!!previous);
  const [error, setError] = useState<unknown>(null);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (sending || !text.trim() || text.length > 12000) return;
    const payload = complaintReplyRequest(
      requests.get(detail.id),
      detail,
      text.trim(),
      status,
    );
    const existing = requests.get(detail.id);
    requests.set(detail.id, payload);
    try {
      responseStore.write(requests);
    } catch (failure) {
      if (existing) requests.set(detail.id, existing);
      else requests.delete(detail.id);
      setError(errorText(failure));
      return;
    }
    setSending(true);
    setError("");
    try {
      const outcome = await submitComplaintResponse(
        payload,
        (request) => post("/api/complaints", request),
        (id) => get("/api/complaint", { query: { id } }),
      );
      const remaining = new Map(requests);
      remaining.delete(detail.id);
      responseStore.write(remaining);
      requests.delete(detail.id);
      setRetry(false);
      await clearReply(payload.text);
      setText((current) => (current === payload.text ? "" : current));
      if ("detail" in outcome) onResponse(outcome.detail);
      else notify(errorText(outcome.reloadError));
      try {
        await refresh();
      } catch (error) {
        notify(errorText(error));
      }
    } catch (failure) {
      if (failure instanceof ApiError) {
        if (isDefinitiveComplaintRefusal(failure.status)) {
          await clearReply(payload.text);
          const remaining = new Map(requests);
          remaining.delete(detail.id);
          try {
            responseStore.write(remaining);
          } catch (storageFailure) {
            setError(errorText(storageFailure));
            return;
          }
          requests.delete(detail.id);
          setRetry(false);
          if (
            failure.status === 409 ||
            /version|changed|stale/i.test(failure.message)
          ) {
            setError(
              "This message changed. Review the latest response before you send again.",
            );
            try {
              await refresh();
              onResponse(
                await get("/api/complaint", { query: { id: detail.id } }),
              );
            } catch (refreshFailure) {
              setError(errorText(refreshFailure));
            }
            return;
          }
          setError(errorText(failure) || "The response was rejected.");
          return;
        }
      }
      const uncertain = requests.has(detail.id);
      setRetry(uncertain);
      setError(
        `${errorText(failure)}${uncertain ? " Retry sends the same response." : ""}`,
      );
    } finally {
      setSending(false);
    }
  };
  return (
    <form className="complaint-reply" onSubmit={submit}>
      <h3>Your reply</h3>
      <Textarea
        label="Reply"
        aria-label="Reply"
        value={text}
        required
        error={
          text.length > 12000
            ? "The reply exceeds 12,000 characters. Shorten it before you send."
            : undefined
        }
        autosize
        minRows={3}
        maxRows={8}
        disabled={sending || retry}
        onChange={(event) => {
          setText(event.target.value);
          saveReply(event.target.value);
        }}
      />
      {!!error && (
        <p className="complaint-reply-error" role="alert">
          <ErrorDescription value={error} role="status" />
        </p>
      )}
      <Button
        type="submit"
        variant="filled"
        loading={sending}
        disabled={!text.trim() || text.length > 12000}
      >
        {retry ? "Retry response" : "Send reply"}
      </Button>
      <p className="notice">The agent receives your response as a message.</p>
    </form>
  );
}

export default observer(function UserMessages({
  target = "user",
  hideEmpty = false,
  focusId,
  onlyComplaintId,
  focusRequestId,
}: {
  target?: "user" | "lead";
  hideEmpty?: boolean;
  focusId?: string | undefined;
  onlyComplaintId?: string;
  focusRequestId?: string | undefined;
}) {
  const storage = useServerStorage();
  const store = useServerStore();
  const state = resourceData(store.state().current);
  const stateDir = state?.workspaces.values().next().value?.stateDir || "";
  const pending = useMemo(() => {
    try {
      return new Map(
        Object.entries(complaintResponseStore(storage, stateDir).read()),
      );
    } catch {
      return new Error(
        "The saved response request cannot be read. Resolve it before sending another response.",
      );
    }
  }, [stateDir, storage]);
  const selected = onlyComplaintId
    ? state?.complaints.get(onlyComplaintId)
    : undefined;
  const records = (
    onlyComplaintId
      ? selected
        ? [selected]
        : []
      : [...(state?.complaints.values() || [])]
  ).filter((message) => message.recipient === target);
  return (
    <section className="user-message-list">
      {pending instanceof Error ? (
        <p role="alert">{pending.message}</p>
      ) : (
        records.map((message) => (
          <UserMessage
            key={JSON.stringify([stateDir, message.id])}
            message={message}
            scope={stateDir}
            focused={message.id === focusId}
            focusRequestId={focusRequestId}
            requests={pending}
          />
        ))
      )}
      {!hideEmpty && !records.length && (
        <p className="notice">No messages yet.</p>
      )}
    </section>
  );
});

export const UserMessage = observer(function UserMessage({
  message,
  scope,
  focused,
  focusRequestId,
  requests,
}: {
  message: ComplaintSummary;
  scope: string;
  focused: boolean;
  focusRequestId?: string | undefined;
  requests: Map<string, UserComplaintResponse>;
}) {
  useMessageRenderProbe("user-message", message.id);
  const { get } = useServerApi();
  const store = useServerStore();
  const confirmedMessages = useMessageCache();

  const confirmedKey = JSON.stringify([scope, message.id]);
  const [reply, setReply] = useState(focused || requests.has(message.id));
  const [error, setError] = useState<unknown>(null);
  const current = store.complaint(complaintId(message.id)).current;
  useEffect(() => {
    if (focused) setReply(true);
  }, [focused, focusRequestId]);
  // Retire only when full authoritative detail has materialized the confirmed version.
  useEffect(
    () =>
      autorun(() => {
        const latest = resourceData(
          store.complaint(complaintId(message.id)).current,
        );
        const confirmed = confirmedMessages.get(confirmedKey);
        if (latest && confirmed && latest.version >= confirmed.version)
          runInAction(() => confirmedMessages.delete(confirmedKey));
      }),
    [store, message.id, confirmedMessages, confirmedKey],
  );
  const confirmed = confirmedMessages.get(confirmedKey);
  const source = resourceData(current) || message;
  const detail =
    confirmed &&
    confirmed.version > (isComplaintDetail(source) ? source.version : -1)
      ? confirmed
      : source;
  const remember = (result: ComplaintDetailResponse) =>
    runInAction(() => {
      const cached = confirmedMessages.get(confirmedKey);
      if (!cached || cached.version <= result.version)
        confirmedMessages.set(confirmedKey, result);
    });
  const retryDetail = () => {
    void get("/api/complaint", { query: { id: message.id } })
      .then((result) => {
        remember(result);
        setError(null);
      })
      .catch(setError);
  };
  const detailView = isComplaintDetail(detail) ? detail : null;
  const detailError = error || ("error" in current ? current.error : null);
  const authorName = (id: string) =>
    id === "user"
      ? "You"
      : resourceData(store.state().current)?.agents.get(id)?.name ||
        message.authorName ||
        id;
  return (
    <article className="message-inline-thread" data-complaint={message.id}>
      <div className="chat-message-author">
        <AgentAvatar id={message.author || ""} size={24} />
        <strong>{authorName(message.author || "")}</strong>
        {message.recipient === "lead" && (
          <span>to {message.leadName || "Main agent"}</span>
        )}
      </div>
      <MessageDate at={detailView?.created ?? message.created} />
      <StreamingText
        text={detailView?.text ?? ""}
        agentId={message.author || undefined}
      />
      {!detailView && !detailError && <p role="status">Loading message…</p>}
      {!!detailError && (
        <p role="alert">
          <ErrorDescription value={detailError} role="status" />{" "}
          <Button onClick={retryDetail}>Retry</Button>
        </p>
      )}
      {detailView?.responses.map((response) => (
        <article className="complaint-response" key={response.id}>
          <strong>
            {authorName(response.author || detailView.leadId || "")}
          </strong>
          <MessageDate at={response.at} />
          <StreamingText
            text={response.text || ""}
            agentId={response.author || undefined}
          />
        </article>
      ))}
      {isComplaintDetail(detail) && detail.recipient === "user" && (
        <>
          <Button
            variant="subtle"
            size="compact-xs"
            aria-expanded={reply}
            onClick={() => setReply(!reply)}
          >
            {reply ? "Close reply" : "Reply"}
          </Button>
          {reply && (
            <MessageResponse
              detail={detail}
              requests={requests}
              scope={scope}
              onResponse={remember}
            />
          )}
        </>
      )}
    </article>
  );
});

function isComplaintDetail(
  complaint: ComplaintSummary | ComplaintDetailResponse,
): complaint is ComplaintDetailResponse {
  return "text" in complaint;
}

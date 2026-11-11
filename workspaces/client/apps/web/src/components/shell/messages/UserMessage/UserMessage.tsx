import { useMessageRenderProbe } from "../../../../perf/messageRenderProbe.ts";
import { useUnsentText } from "../../../../unsent/useUnsentText.ts";
import {
  complaintId,
  type ComplaintDetailResponse,
  type StateCollections,
} from "../../../../generated/observableResources.ts";
import { resourceData } from "../../../../store/runtime.ts";
import { useConversationActions } from "../../../conversation/support/navigation.ts";
import {
  useServerApi,
  useServerStorage,
  useServerStore,
} from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button, Textarea } from "@mantine/core";
import { useState } from "react";
import { observer } from "mobx-react-lite";
import { ApiError, errorText } from "../../../../api.ts";

import ErrorDescription from "../../ErrorDescription/ErrorDescription.tsx";
import MessageDate from "../../../conversation/transcript/MessageDate/MessageDate.tsx";
import StreamingText from "../../../conversation/transcript/StreamingText/StreamingText.tsx";
import AgentAvatar from "../../../agents/AgentAvatar/AgentAvatar.tsx";
import {
  complaintReplyRequest,
  complaintResponseStore,
  isDefinitiveComplaintRefusal,
  submitComplaintResponse,
  type UserComplaintResponse,
} from "../support/complaintReplyRequest.ts";

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
  const ui = useConversationActions().ui;
  const confirmedMessages = ui.messageCache;

  const confirmedKey = JSON.stringify([scope, message.id]);
  const [reply, setReply] = useState<{
    request: string | undefined;
    open: boolean;
  } | null>(null);
  const replyOpen =
    reply && reply.request === focusRequestId
      ? reply.open
      : focused || requests.has(message.id);
  const [error, setError] = useState<unknown>(null);
  const current = store.complaint(complaintId(message.id)).current;
  const confirmed = confirmedMessages.get(confirmedKey);
  const source = resourceData(current) || message;
  const detail =
    confirmed &&
    confirmed.version > (isComplaintDetail(source) ? source.version : -1)
      ? confirmed
      : source;
  const remember = (result: ComplaintDetailResponse) =>
    ui.rememberComplaint(confirmedKey, result);
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
            aria-expanded={replyOpen}
            onClick={() =>
              setReply({ request: focusRequestId, open: !replyOpen })
            }
          >
            {replyOpen ? "Close reply" : "Reply"}
          </Button>
          {replyOpen && (
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

// Preserve confirmed replies while a cached snapshot catches up.
type ComplaintSummary =
  StateCollections["complaints"] extends ReadonlyMap<string, infer T>
    ? T
    : never;

const MessageResponse = observer(function MessageResponse({
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
  const { notify } = area.notices;
  const storage = useServerStorage();
  const responseStore = complaintResponseStore(storage, scope);

  const draftKey = JSON.stringify([
    `studio-message-responses:${scope}`,
    detail.id,
  ]);
  const [savedReply, saveReply, clearReply] = useUnsentText("reply", draftKey);
  const previous = requests.get(detail.id);
  const [edited, setText] = useState<string | undefined>(previous?.text);
  const text = edited ?? savedReply ?? "";
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
      setText((current) =>
        (current ?? savedReply ?? "") === payload.text ? "" : current,
      );
      if ("detail" in outcome) onResponse(outcome.detail);
      else notify(errorText(outcome.reloadError));
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
});

function isComplaintDetail(
  complaint: ComplaintSummary | ComplaintDetailResponse,
): complaint is ComplaintDetailResponse {
  return "text" in complaint;
}

import { Owned } from "../../../ui/Owned/Owned.tsx";
import type { ComplaintSummary } from "../../../../app/shell/AppUi.tsx";
import { useState } from "react";
import { createComplaintReply, complaintDraftKey } from "./ComplaintReply.ts";
import { useMessageRenderProbe as reportMessageRender } from "../../../../perf/messageRenderProbe.ts";

import { type ComplaintDetailResponse } from "../../../../generated/observableResources.ts";

import { useConversationActions } from "../../../conversation/support/navigation.ts";
import {
  useServerApi,
  useServerStorage,
  useServerStore,
} from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button, Textarea } from "@mantine/core";

import { observer } from "mobx-react-lite";

import ErrorDescription from "../../ErrorDescription/ErrorDescription.tsx";
import MessageDate from "../../../conversation/transcript/MessageDate/MessageDate.tsx";
import StreamingText from "../../../conversation/transcript/StreamingText/StreamingText.tsx";
import AgentAvatar from "../../../agents/AgentAvatar/AgentAvatar.tsx";
import { type UserComplaintResponse } from "../support/complaintReplyRequest.ts";

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
  const api = useServerApi();
  const store = useServerStore();
  const ui = useConversationActions().ui;
  const [reply, setReply] = useState<{
    request: string | undefined;
    open: boolean;
  } | null>(null);
  const replyOpen =
    reply?.request === focusRequestId && reply !== null
      ? reply.open
      : focused || requests.has(message.id);
  const { key, detailView, detailError, authorName, remember } =
    ui.complaintView(message, scope, store);
  reportMessageRender("user-message", message.id);
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
          <Button onClick={() => void ui.retryComplaint(api, message.id, key)}>
            Retry
          </Button>
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
      {detailView && detailView.recipient === "user" && (
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
              detail={detailView}
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
  const api = useServerApi();
  const area = useConversationActions();
  const storage = useServerStorage();
  const savedReply = area.unsentText.read(
    "reply",
    complaintDraftKey(scope, detail.id),
  );
  return (
    <Owned
      key={JSON.stringify([scope, detail.id])}
      create={() => createComplaintReply(requests.get(detail.id))}
    >
      {(reply) => {
        const text = reply.text(savedReply);
        return (
          <form
            className="complaint-reply"
            onSubmit={(event) => {
              event.preventDefault();
              void reply.submit({
                area,
                api,
                storage,
                scope,
                detail,
                requests,
                onResponse,
              });
            }}
          >
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
              disabled={reply.sending || reply.retry}
              onChange={(event) => {
                reply.edit(event.target.value, area, scope, detail.id);
              }}
            />
            {!!reply.error && (
              <p className="complaint-reply-error" role="alert">
                <ErrorDescription value={reply.error} role="status" />
              </p>
            )}
            <Button
              type="submit"
              variant="filled"
              loading={reply.sending}
              disabled={reply.invalid(savedReply)}
            >
              {reply.retry ? "Retry response" : "Send reply"}
            </Button>
            <p className="notice">
              The agent receives your response as a message.
            </p>
          </form>
        );
      }}
    </Owned>
  );
});

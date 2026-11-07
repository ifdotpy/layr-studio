import { RadioStatus } from "../generated/api";
import { serverMutationResponseIsUncertain } from "../servers/mutationIdentity";
import { useMessageRenderProbe } from "./shell/messages/renderProbe";
import { randomUUID } from "../uuid";
import { useServerApi, useServerStore } from "../servers/ServerServicesContext";
import { providerLabel } from "../generated/providerKind.ts";
import { Button, Select, Textarea } from "@mantine/core";
import { useRef, useState } from "react";
import { ApiError, errorText, type PostBody } from "../api";
import { useRoomHistory } from "../conversation/roomHistory";
import { resourceData } from "../store/runtime";
import { observer, Observer } from "mobx-react-lite";
import { useConversationActions } from "./conversation/navigation";
import type { StateCollections } from "../generated/observableResources";
import AgentAvatar from "./agents/AgentAvatar";
import MessageDate from "./conversation/transcript/MessageDate";
import Requests, { RequestArea } from "./questions/Requests";
import StreamingText from "./conversation/transcript/StreamingText";
import { useConversationScroll } from "./useConversationScroll";
import PromptComposer from "./prompt-composer/PromptComposer";
import "./radio-chat.css";

// Keep the exact command across navigation and reloads until the server confirms it.
type PeerTeamRequest = PostBody<"/api/peer-teams">;
type RadioRequest = Extract<
  PeerTeamRequest,
  { action: "radio"; radio_action: "send" | "pass" | "stop" }
>;
type RadioActionInput =
  | { action: "pass"; target: string }
  | { action: "stop" }
  | { action: "send"; text: string; target: string; rounds: 1 | 2 };
type Attempt = {
  body: RadioRequest;
  acknowledged?: boolean;
  rejected?: boolean;
};
export default observer(function RadioChat({
  roomId,
  visible = true,
}: {
  roomId: string;
  visible?: boolean;
}) {
  const store = useServerStore();
  const state = resourceData(store.state().current);
  const room = state?.rooms.get(roomId);
  if (!state || !room?.radio) return <p role="status">Loading shared chat…</p>;
  return <RadioScreen room={room} state={state} visible={visible} />;
});

type RadioRoom =
  StateCollections["rooms"] extends ReadonlyMap<string, infer T> ? T : never;
const RadioScreen = observer(function RadioScreen({
  room,
  state,
  visible,
}: {
  room: RadioRoom;
  state: StateCollections;
  visible: boolean;
}) {
  useMessageRenderProbe("radio-screen", room.id);
  const { post, save, saved } = useServerApi();
  const { getDraft, subscribeDraft, setDraft, refresh, notify } =
    useConversationActions();
  const radio = room.radio!;
  const stateDir = state.workspaces.values().next().value?.stateDir || "";
  const key = `studio-radio-command:${stateDir}:${room.id}`;
  const [attempt, setAttempt] = useState<Attempt | null>(() =>
    saved(key, null),
  );
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [target, setTarget] = useState("both");
  const locked = useRef(false);
  const members = (room.members || [])
    .map((id) => state.agents.get(id))
    .filter((a) => !!a);
  const messages = useRoomHistory(room.id, stateDir, visible);
  const { scroll, content, onScroll } = useConversationScroll(
    room.id,
    messages.loaded,
  );
  const remember = (value: Attempt | null) => {
    save(key, value);
    setAttempt(value);
  };
  const execute = async (next: Attempt) => {
    if (locked.current) return;
    locked.current = true;
    setPending(true);
    setError("");
    remember(next);
    try {
      if (!next.acknowledged && !next.rejected) {
        await post("/api/peer-teams", next.body, { timeoutMs: 15000 });
        next = { ...next, acknowledged: true };
        remember(next);
      }
      if (
        next.acknowledged &&
        next.body.radio_action === "send" &&
        getDraft(room.id) === next.body.text
      )
        setDraft("", room.id);
      if (next.rejected) await refresh();
      messages.reload();
      remember(null);
      if (next.acknowledged)
        void refresh().catch((error) => notify(errorText(error)));
    } catch (e) {
      if (
        !next.acknowledged &&
        e instanceof ApiError &&
        !serverMutationResponseIsUncertain(e.status, e.response)
      ) {
        next = { ...next, rejected: true };
        remember(next);
      }
      setError(errorText(e));
    } finally {
      locked.current = false;
      setPending(false);
    }
  };
  const command = (input: RadioActionInput) => {
    if (attempt || locked.current) return;
    const path =
      room.projectPath || state.peerTeams.get(radio.teamId)?.projectPath;
    if (!path) {
      notify("The shared chat project is unavailable. Refresh Studio.");
      return;
    }
    const teamId = radio.teamId || room.peerTeamId;
    if (!teamId) {
      notify("The shared chat identity is unavailable. Refresh Studio.");
      return;
    }
    const base = {
      action: "radio" as const,
      path,
      team_id: teamId,
      request_id: randomUUID(),
      expected_revision: radio.revision,
    };
    const body: RadioRequest =
      input.action === "stop"
        ? { ...base, radio_action: "stop" }
        : input.action === "pass"
          ? { ...base, radio_action: "pass", target: input.target }
          : {
              ...base,
              radio_action: "send",
              text: input.text,
              target: input.target,
              rounds: input.rounds,
            };
    void execute({ body });
  };
  const { busy, inputBlocked, canPass, stopping, speaking } = radio.controls;
  const busyNotice = !busy
    ? null
    : radio.status === RadioStatus.blocked
      ? "Resolve the current reply before sending."
      : radio.status === RadioStatus.stopping
        ? "Waiting for the reply to stop."
        : "Current reply finishes first.";
  const frozen = pending || !!attempt;
  return (
    <section className="radio-chat" aria-label="Shared chat">
      <div className="radio-participants">
        {members.map((agent) => (
          <div
            className="radio-participant"
            key={agent.id}
            data-speaking={radio.speaker === agent.id && speaking}
          >
            <AgentAvatar id={agent.id} size={28} />
            <div>
              <strong>{agent.name}</strong>
              <small>
                {providerLabel(agent.provider || "")} · {agent.model}
              </small>
            </div>
            <span className="radio-speaker">
              {radio.speaker === agent.id && speaking
                ? "Speaking"
                : busy
                  ? "Waiting"
                  : "Ready"}
            </span>
            {canPass && radio.speaker !== agent.id && (
              <Button
                size="compact-xs"
                variant="subtle"
                disabled={frozen || stopping}
                onClick={() => command({ action: "pass", target: agent.id })}
              >
                Reply next
              </Button>
            )}
          </div>
        ))}
        {busy && (
          <Button
            size="compact-xs"
            variant="subtle"
            disabled={frozen || stopping}
            onClick={() => command({ action: "stop" })}
          >
            {stopping ? "Stopping…" : "Stop"}
          </Button>
        )}
      </div>
      <div className="radio-history" ref={scroll} onScroll={onScroll}>
        <div className="radio-history-content" ref={content}>
          {messages.before != null && (
            <Button
              variant="subtle"
              size="compact-xs"
              disabled={messages.pageLoading}
              onClick={() => void messages.older()}
            >
              Load earlier messages
            </Button>
          )}
          {messages.notice && <p role="status">{messages.notice}</p>}
          {!messages.loaded && <p role="status">Loading messages…</p>}
          {messages.loaded && !messages.items.length && (
            <p className="radio-empty">
              Both agents see this conversation. They reply one at a time.
            </p>
          )}
          {messages.items.map((message) => (
            <article
              className="radio-message"
              data-sender={message.sender}
              data-message={message.id}
              key={message.id}
            >
              <div className="radio-author">
                {message.sender !== "user" && (
                  <AgentAvatar id={message.sender || "agent"} size={24} />
                )}
                <strong>
                  {message.sender === "user"
                    ? "You"
                    : message.senderName ||
                      members.find((a) => a.id === message.sender)?.name ||
                      "Agent"}
                </strong>
                <MessageDate at={message.created} />
              </div>
              <StreamingText
                text={message.text || ""}
                agentId={message.sender}
              />
            </article>
          ))}
          <RequestArea scope={stateDir} refresh={refresh} notify={notify}>
            <Requests
              agentId={radio.active?.agentId}
              turnId={radio.active?.turnId}
              pendingOnly
            />
          </RequestArea>
        </div>
      </div>
      <PromptComposer
        session={room.id}
        getDraft={getDraft}
        subscribeDraft={subscribeDraft}
        className="radio-composer"
        onSubmit={(e) => {
          e.preventDefault();
          const text = getDraft(room.id);
          if (!frozen && !inputBlocked && text.trim())
            command({ action: "send", text, target, rounds: 1 });
        }}
      >
        {(draft) => {
          const canSend = !frozen && !inputBlocked && !!draft.trim();
          return (
            <Observer>
              {() => (
                <>
                  {radio.error && <p role="alert">{radio.error}</p>}
                  {attempt && (
                    <div className="radio-retry" role="alert">
                      <span>
                        {error || "A previous request needs confirmation."}{" "}
                        {attempt.acknowledged
                          ? "Saved. Refresh to continue."
                          : attempt.rejected
                            ? "Request rejected. Refresh to continue."
                            : "The result is unknown. Retry the same request."}
                      </span>
                      <Button
                        size="compact-xs"
                        disabled={pending}
                        onClick={() => void execute(attempt)}
                      >
                        {attempt.acknowledged || attempt.rejected
                          ? "Refresh"
                          : "Retry"}
                      </Button>
                    </div>
                  )}
                  <Textarea
                    aria-label="Message both agents"
                    placeholder="Message the shared chat…"
                    autosize
                    minRows={2}
                    maxRows={7}
                    value={draft}
                    disabled={frozen}
                    onChange={(e) => setDraft(e.currentTarget.value, room.id)}
                    onKeyDown={(e) => {
                      if (
                        e.key === "Enter" &&
                        !e.shiftKey &&
                        !e.nativeEvent.isComposing &&
                        canSend
                      ) {
                        e.preventDefault();
                        command({
                          action: "send",
                          text: draft,
                          target,
                          rounds: 1,
                        });
                      }
                    }}
                  />
                  <div className="radio-compose-actions">
                    <Select
                      aria-label="Reply from"
                      data={[
                        { value: "both", label: "Both agents" },
                        ...members.map((a) => ({
                          value: a.id,
                          label: a.name || "Agent",
                        })),
                      ]}
                      value={target}
                      onChange={(value) => setTarget(value || "both")}
                      allowDeselect={false}
                      disabled={frozen || inputBlocked}
                    />
                    <span>
                      {busyNotice ??
                        (target === "both" ? "One reply each" : "One reply")}
                    </span>
                    {target === "both" && (
                      <Button
                        variant="subtle"
                        disabled={!canSend}
                        title="Two replies each, four replies total"
                        onClick={() =>
                          command({
                            action: "send",
                            text: draft,
                            target,
                            rounds: 2,
                          })
                        }
                      >
                        Discuss (4 replies)
                      </Button>
                    )}
                    <Button type="submit" disabled={!canSend}>
                      Send
                    </Button>
                  </div>
                </>
              )}
            </Observer>
          );
        }}
      </PromptComposer>
    </section>
  );
});

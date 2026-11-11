import { Owned } from "../../ui/Owned/Owned.tsx";
import { RadioDelivery, type RadioRoom } from "./RadioDelivery.ts";
import { RadioStatus } from "../../../generated/api.ts";
import { useMessageRenderProbe } from "../../../perf/messageRenderProbe.ts";
import {
  useServerApi,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { providerLabel } from "../../../generated/providerKind.ts";
import { Button, Select, Textarea } from "@mantine/core";
import { resourceData } from "../../../store/runtime.ts";
import { observer } from "mobx-react-lite";
import { useConversationActions } from "../support/navigation.ts";
import AgentAvatar from "../../agents/AgentAvatar/AgentAvatar.tsx";
import MessageDate from "../transcript/MessageDate/MessageDate.tsx";
import Requests from "../questions/Requests/Requests.tsx";
import { RequestArea } from "../questions/RequestArea/RequestArea.tsx";
import StreamingText from "../transcript/StreamingText/StreamingText.tsx";
import PromptComposer from "../prompt-composer/PromptComposer/PromptComposer.tsx";

export default observer(function RadioChat({ room }: { room: RadioRoom }) {
  const store = useServerStore();
  const state = resourceData(store.state().current);
  const api = useServerApi();
  const area = useConversationActions();
  const messages = area.selected.history;
  const { scroll, content, onScroll } = area.selected.roomScrolling;
  useMessageRenderProbe("radio-screen", room.id);
  if (!state || !room?.radio) return <p role="status">Loading shared chat…</p>;
  const key = `studio-radio-command:${state.workspaces.values().next().value?.stateDir || ""}:${room.id}`;
  return (
    <Owned
      key={key}
      create={() =>
        new RadioDelivery({ roomId: room.id, key }, api.saved(key, null))
      }
    >
      {(delivery) => {
        const { getDraft } = area.scope.services.drafts;
        const { editDraft: setDraft } = area.delivery;
        const radio = room.radio!;
        const members = (room.members || [])
          .map((id) => state.agents.get(id))
          .filter((a) => !!a);
        const { attempt, pending, error, target, frozen } = delivery;
        const command = (input: Parameters<RadioDelivery["command"]>[0]) =>
          delivery.command(input, room, state, { api, area });
        const send = (text: string, rounds: 1 | 2 = 1) =>
          command({ radio_action: "send", text, target, rounds });
        const { busy, inputBlocked, canPass, stopping, speaking } =
          radio.controls;
        const busyNotice = !busy
          ? null
          : radio.status === RadioStatus.blocked
            ? "Resolve the current reply before sending."
            : radio.status === RadioStatus.stopping
              ? "Waiting for the reply to stop."
              : "Current reply finishes first.";
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
                      onClick={() =>
                        command({ radio_action: "pass", target: agent.id })
                      }
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
                  onClick={() => command({ radio_action: "stop" })}
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
                            members.find((a) => a.id === message.sender)
                              ?.name ||
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
                <RequestArea owner={area.selected.requestOwner}>
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
              className="radio-composer"
              onSubmit={(e) => {
                e.preventDefault();
                send(getDraft(room.id));
              }}
            >
              {(draft) => {
                const canSend = !frozen && !inputBlocked && !!draft.trim();
                return (
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
                          onClick={() =>
                            void delivery.execute(attempt, { api, area })
                          }
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
                          send(draft);
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
                        onChange={delivery.setTarget}
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
                          onClick={() => send(draft, 2)}
                        >
                          Discuss (4 replies)
                        </Button>
                      )}
                      <Button type="submit" disabled={!canSend}>
                        Send
                      </Button>
                    </div>
                  </>
                );
              }}
            </PromptComposer>
          </section>
        );
      }}
    </Owned>
  );
});

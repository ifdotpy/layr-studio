import { useMessageRenderProbe } from "../../../../perf/messageRenderProbe.ts";
import { RoomKind as GeneratedApiRoomKind } from "../../../../generated/api.ts";
import MessageDate from "../../../conversation/transcript/MessageDate/MessageDate.tsx";
import { localDateTime, localTime } from "../../../../local-time.ts";
import AgentAvatar from "../../../agents/AgentAvatar/AgentAvatar.tsx";
import {
  ActionIcon,
  Button,
  Loader,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import { useEffect, useLayoutEffect, useRef } from "react";
import { ArrowLeft, Inbox, Megaphone, Search, Users } from "lucide-react";
import "../../../../api.ts";

import { observer } from "mobx-react-lite";
import {
  type TeamChatsOwner,
  TeamChatsProvider,
  useTeamChats,
  preview,
  type MessageRoom,
} from "./TeamChatsStore.ts";
import { useConversationActions } from "../../../conversation/support/navigation.ts";
import StreamingText from "../../../conversation/transcript/StreamingText/StreamingText.tsx";
import Requests, {
  RequestArea,
} from "../../../conversation/questions/Requests/Requests.tsx";
import UserMessages from "../UserMessages/UserMessages.tsx";

/** A compact room clock; its native tooltip retains the full local date. */
function RoomDate({ at }: { at?: number | null | undefined }) {
  if (!at || !Number.isFinite(at)) return <MessageDate at={at} />;
  const date = new Date(at * 1000);
  return (
    <time
      className="message-date"
      dateTime={date.toISOString()}
      title={localDateTime(date)}
    >
      {date.toDateString() === new Date().toDateString()
        ? localTime(date, { hour: "2-digit", minute: "2-digit" })
        : localDateTime(date, { month: "short", day: "numeric" })}
    </time>
  );
}

type Props = { owner?: TeamChatsOwner };
export default observer(function TeamChats(props: Props) {
  useMessageRenderProbe("team-chats");
  const area = useConversationActions();
  const owner = props.owner ?? area.ui.chats;
  const chats = owner.current;
  if (!chats.messages) return null;
  return (
    <TeamChatsProvider value={chats}>
      <div className="team-chats-container">
        <section
          className={
            "team-chats messages-chat-list " + (chats.detail ? "show-room" : "")
          }
          aria-label="Messages"
        >
          <RoomList />
          <TeamDetail owner={owner} />
        </section>
      </div>
    </TeamChatsProvider>
  );
});

const TeamDetail = observer(function TeamDetail({
  owner,
}: {
  owner: TeamChatsOwner;
}) {
  const chats = useTeamChats();
  const {
    id: focusItemId,
    requestId: focusRequestId,
    roomId: focusRoomId,
  } = chats.target ?? {};
  const { room, selected, detail: detailOpen, leadId, back: onBack } = chats;
  const {
    notices: { notify },
  } = useConversationActions();
  useMessageRenderProbe("team-detail", room?.id || "you");
  const showYou = selected === "you" || !room;
  const messages = owner.messages;
  if (!messages)
    throw new Error("Room session must be active inside TeamChats");
  const detailHeading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (detailOpen) detailHeading.current?.focus({ preventScroll: true });
  }, [detailOpen, selected]);
  const { scroll, content, getFollow, setFollow, onScroll } = chats.scrolling;
  const follow = getFollow();
  const virtualRange = chats.viewport.range;
  useLayoutEffect(() => {
    const root = scroll.current;
    if (!root || showYou) return;
    const update = () => chats.measureRoom(root);
    update();
    root.addEventListener("scroll", update, { passive: true });
    const observer = new ResizeObserver(update);
    observer.observe(root);
    root
      .querySelectorAll<HTMLElement>("[data-room-row]")
      .forEach((row) => observer.observe(row));
    return () => {
      observer.disconnect();
      root.removeEventListener("scroll", update);
    };
  }, [
    detailOpen,
    messages.notice,
    messages.items,
    showYou,
    messages.loaded,
    virtualRange.start,
    virtualRange.end,
  ]);
  const handleScroll = () => {
    onScroll();
    if (showYou) return;
    const root = scroll.current;
    if (!root) return;
    const bounds = root.getBoundingClientRect();
    const anchor = Array.from(
      root.querySelectorAll<HTMLElement>("[data-message]"),
    ).find((node) => {
      const rect = node.getBoundingClientRect();
      return rect.bottom > bounds.top && rect.top < bounds.bottom;
    });
    messages.setPageAnchor(anchor?.dataset.message || null);
  };
  useEffect(() => {
    if ((!showYou && !messages.loaded) || !focusItemId) return;
    const target = content.current?.querySelector(
      `[data-feed-item="request:${CSS.escape(focusItemId)}"], [data-feed-item="complaint:${CSS.escape(focusItemId)}"]`,
    );
    if (target) {
      setFollow(false);
      target.scrollIntoView({ block: "center" });
    }
  }, [messages.loaded, selected, focusItemId, focusRequestId, focusRoomId]);
  return (
    <div className="team-room-detail unified-messages">
      <header className="team-room-header">
        <ActionIcon
          className="team-room-back"
          variant="subtle"
          aria-label="Back to chats"
          onClick={() => onBack()}
        >
          <ArrowLeft size={20} />
        </ActionIcon>
        {roomIcon(showYou ? undefined : room, leadId)}
        <div>
          <h2 ref={detailHeading} tabIndex={-1}>
            {showYou ? "For you" : chats.name(room!)}
          </h2>
          <span className="team-room-kind">
            {showYou
              ? "Questions, messages and replies"
              : room?.kind === GeneratedApiRoomKind.broadcast
                ? "Team conversation"
                : room?.kind === GeneratedApiRoomKind.federated
                  ? `Remote room on ${room.peerLabel || "paired server"}`
                  : "Conversation"}
          </span>
        </div>
      </header>
      <div
        className="unified-message-scroll"
        data-room-retained={!showYou ? messages.items.length : undefined}
        ref={scroll}
        onScroll={handleScroll}
      >
        <div ref={content}>
          {(["older", "newer"] as const).map(
            (direction) =>
              !showYou &&
              messages[direction === "older" ? "before" : "after"] != null && (
                <Button
                  key={direction}
                  variant="subtle"
                  size="xs"
                  onClick={() => {
                    setFollow(false);
                    void messages[direction]().catch((error) =>
                      notify(String(error)),
                    );
                  }}
                >
                  {direction === "older"
                    ? "Earlier messages"
                    : "Later messages"}
                </Button>
              ),
          )}
          {!showYou && !messages.loaded && <Loader size="sm" />}
          {!showYou && messages.notice && (
            <p role="alert">
              {messages.notice}{" "}
              <Button variant="subtle" onClick={() => void messages.reload()}>
                Retry
              </Button>
            </p>
          )}
          {showYou ? (
            <PersonalFeed
              focusItemId={focusItemId}
              focusRequestId={focusRequestId}
            />
          ) : (
            <>
              <div aria-hidden="true" style={{ height: virtualRange.top }} />
              {messages.items
                .slice(virtualRange.start, virtualRange.end)
                .map((message) => (
                  <div
                    key={message.id}
                    data-room-row={message.id}
                    data-room-seq={message.seq}
                  >
                    <article className="team-message" data-message={message.id}>
                      <div className="chat-message-author">
                        <AgentAvatar id={message.sender || "agent"} size={24} />
                        <strong>{message.senderName || "Agent"}</strong>
                      </div>
                      <StreamingText
                        text={message.text || ""}
                        agentId={message.sender}
                      />
                      <MessageDate at={message.created} />
                    </article>
                  </div>
                ))}
              <div aria-hidden="true" style={{ height: virtualRange.bottom }} />
            </>
          )}
          {!showYou && messages.loaded && !messages.items.length && (
            <div className="team-chat-empty">
              <Inbox aria-hidden="true" />
              <strong>No messages yet.</strong>
              <p>
                Messages between teammates will appear here as they coordinate
                their work. You can read their decisions and follow the
                handoffs.
              </p>
            </div>
          )}
        </div>
      </div>
      {!follow && (
        <Button
          className="team-latest"
          size="compact-xs"
          onClick={() => {
            messages.showLatest();
            setFollow(true);
          }}
        >
          Latest messages
        </Button>
      )}
    </div>
  );
});

function roomIcon(item?: MessageRoom, leadId?: string) {
  return !item ? (
    <span className="team-conversation-icon personal">
      <Inbox size={21} />
    </span>
  ) : item.kind === GeneratedApiRoomKind.broadcast ||
    item.kind === GeneratedApiRoomKind.federated ||
    (item.members ?? []).length > 2 ||
    !(item.members ?? []).includes(leadId || "") ? (
    <span className="team-conversation-icon">
      {item.kind === GeneratedApiRoomKind.broadcast ? (
        <Megaphone size={21} />
      ) : (
        <Users size={21} />
      )}
    </span>
  ) : (
    <AgentAvatar
      id={(item.members ?? []).find((id) => id !== leadId) || item.id}
      size={42}
    />
  );
}

const RoomRow = observer(function RoomRow({ item }: { item?: MessageRoom }) {
  const chats = useTeamChats();
  const { seen, leadId, choose } = chats;
  const selected = chats.room?.id || "you";
  const latest = item ? undefined : chats.latest;
  const attention = !item && chats.attention;
  const unread =
    !!item?.lastMessage && (seen.get(item.id) || 0) < item.lastMessage.seq;
  useMessageRenderProbe(item ? "room-row" : "personal-row", item?.id);
  const id = item?.id || "you";
  return (
    <UnstyledButton
      data-room={id}
      data-studio-state={attention ? "answer" : unread ? "unread" : "none"}
      className={"team-room-row " + (selected === id ? "selected" : "")}
      aria-pressed={selected === id}
      onClick={() => choose(id)}
    >
      {roomIcon(item, leadId)}
      <div className="team-room-copy">
        <div className="team-room-title">
          <strong>{item ? chats.name(item) : "For you"}</strong>
          <RoomDate at={item?.lastMessage?.created ?? latest?.created} />
        </div>
        <div className="team-room-preview">
          <span>
            {item
              ? item.lastMessage?.text || "No messages yet"
              : preview(latest)}
          </span>
          {(unread || attention) && (
            <i
              className="team-room-unread"
              aria-label={attention ? "Needs your reply" : "Unread messages"}
            />
          )}
        </div>
      </div>
    </UnstyledButton>
  );
});
const RoomList = observer(function RoomList() {
  useMessageRenderProbe("room-list");
  const chats = useTeamChats();
  const { query, limit, filtered } = chats;
  return (
    <nav className="team-room-list" aria-label="Message chats">
      <div className="team-room-search">
        <TextInput
          aria-label="Search chats"
          placeholder="Search message rooms"
          leftSection={<Search size={16} />}
          value={query}
          onChange={(event) => {
            chats.search(event.currentTarget.value);
          }}
        />
      </div>
      <div className="team-room-rows">
        {chats.personal && <RoomRow />}
        {filtered.slice(0, limit).map((item) => (
          <RoomRow key={item.id} item={item} />
        ))}
        {filtered.length > limit && (
          <Button variant="subtle" fullWidth onClick={chats.more}>
            More chats
          </Button>
        )}
        {query && !filtered.length && !chats.personal && (
          <p className="team-chat-empty">
            No rooms match “{query}”. Try an agent or team name.
          </p>
        )}
      </div>
    </nav>
  );
});
const PersonalFeed = observer(function PersonalFeed({
  focusItemId,
  focusRequestId,
}: {
  focusItemId: string | undefined;
  focusRequestId: string | undefined;
}) {
  const area = useConversationActions();
  useMessageRenderProbe("personal-feed");
  const { feed: items } = useTeamChats();
  return (
    <RequestArea owner={area.selected.requestOwner}>
      {items.map((item) => {
        const request = "request" in item;
        const id = (request ? "request:" : "complaint:") + item.id;
        return (
          <div key={id} data-feed-item={id}>
            {request ? (
              <Requests all requestIds={[item.id]} />
            ) : (
              <UserMessages
                onlyComplaintId={item.id}
                target={item.recipient === "lead" ? "lead" : "user"}
                hideEmpty
                focusId={focusItemId}
                focusRequestId={focusRequestId}
              />
            )}
          </div>
        );
      })}
      {!items.length && <p className="team-chat-empty">No messages yet.</p>}
    </RequestArea>
  );
});

export { createTeamChatsStore, type TeamChatsOwner } from "./TeamChatsStore.ts";

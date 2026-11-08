import { useMessageRenderProbe } from "../../../../perf/messageRenderProbe.ts";
import { RoomKind as GeneratedApiRoomKind } from "../../../../generated/api.ts";
import {
  useServerApi,
  useServerStore,
} from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
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
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { ArrowLeft, Inbox, Megaphone, Search, Users } from "lucide-react";
import "../../../../api.ts";
import { useRoomHistory } from "../../../../conversation/roomHistory.ts";
import {
  compareShallow,
  computed,
  observable,
  runInAction,
  type ObservableMap,
} from "mobx";
import { observer } from "mobx-react-lite";
import type { StateCollections } from "../../../../generated/observableResources.ts";
import { approvalSummary, isPending } from "../../../../approvals.ts";
import type { RequestModel } from "../../../conversation/questions/RequestArea/RequestArea.tsx";
import { resourceData } from "../../../../store/runtime.ts";
import { useConversationActions } from "../../../conversation/support/navigation.ts";
import { useRequestsView } from "../../../conversation/support/requestView.ts";
import StreamingText from "../../../conversation/transcript/StreamingText/StreamingText.tsx";
import Requests, {
  RequestArea,
} from "../../../conversation/questions/Requests/Requests.tsx";
import UserMessages from "../UserMessages/UserMessages.tsx";
import {
  useConversationScroll,
  useFollowState,
} from "../../../conversation/support/useConversationScroll.ts";

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

type Props = {
  leadId?: string | undefined;
  focusRequestId?: string | undefined;
  focusItemId?: string | undefined;
  focusRoomId?: string | undefined;
};
export default observer(function TeamChats(props: Props) {
  useMessageRenderProbe("team-chats");
  const store = useServerStore();
  const state = resourceData(store.state().current);
  const rooms = useMemo(
    () =>
      computed(
        () => eligibleRooms(resourceData(store.state().current), props.leadId),
        { equals: compareShallow },
      ),
    [store, props.leadId],
  );
  const stateDir = state?.workspaces.values().next().value?.stateDir || "";
  const scope = stateDir + ":" + props.leadId;
  const { saved } = useServerApi();
  const [seen] = useState(() => observable.map<string, number>());
  useEffect(() => {
    runInAction(() => seen.replace(saved("codex-chat-seen:" + stateDir, {})));
  }, [seen, stateDir, saved]);
  const [selection, setSelection] = useState({
    scope,
    id: "you",
    detail: false,
  });
  const selected = selection.scope === scope ? selection.id : "you";
  const choose = useCallback(
    (id: string) => setSelection({ scope, id, detail: true }),
    [scope],
  );
  useEffect(() => {
    if (props.focusRoomId) choose(props.focusRoomId);
    else if (props.focusItemId || props.focusRequestId) choose("you");
  }, [choose, props.focusRoomId, props.focusItemId, props.focusRequestId]);
  const room = rooms.get().find((room) => room.id === selected);
  return (
    <div className="team-chats-container">
      <section
        className={
          "team-chats messages-chat-list " +
          (selection.scope === scope && selection.detail ? "show-room" : "")
        }
        aria-label="Messages"
      >
        <RoomList
          rooms={rooms.get()}
          state={state}
          seen={seen}
          scope={scope}
          selected={room?.id || "you"}
          leadId={props.leadId}
          choose={choose}
        />
        <TeamDetail
          {...props}
          state={state}
          stateDir={stateDir}
          seen={seen}
          room={room}
          selected={selected}
          scope={scope}
          detailOpen={selection.detail}
          onBack={() => setSelection({ scope, id: selected, detail: false })}
        />
      </section>
    </div>
  );
});

const TeamDetail = observer(function TeamDetail({
  state,
  stateDir,
  seen,
  room,
  selected,
  scope,
  detailOpen,
  onBack,
  leadId,
  focusItemId,
  focusRequestId,
  focusRoomId,
}: Props & {
  state: StateCollections | undefined;
  stateDir: string;
  seen: ObservableMap<string, number>;
  room: MessageRoom | undefined;
  selected: string;
  scope: string;
  detailOpen: boolean;
  onBack(): void;
}) {
  const {
    notices: { notify },
  } = useConversationActions();
  useMessageRenderProbe("team-detail", room?.id || "you");
  const showYou = selected === "you" || !room;
  const messages = useRoomHistory(!showYou && room ? room.id : null, stateDir);
  const { save, saved } = useServerApi();
  const sequence = room?.lastMessage?.seq;
  useEffect(() => {
    if (
      !messages.loaded ||
      !detailOpen ||
      !room ||
      sequence === undefined ||
      (seen.get(room.id) || 0) >= sequence
    )
      return;
    const key = "codex-chat-seen:" + stateDir;
    const next = {
      ...saved<Record<string, number>>(key, {}),
      ...Object.fromEntries(seen),
      [room.id]: sequence,
    };
    save(key, next);
    runInAction(() => {
      seen.replace(next);
    });
  }, [seen, room, sequence, messages.loaded, detailOpen, save, saved]);
  const detailHeading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (detailOpen) detailHeading.current?.focus({ preventScroll: true });
  }, [detailOpen, selected]);
  const { scroll, content, getFollow, subscribeFollow, setFollow, onScroll } =
    useConversationScroll(scope + ":messages:" + selected, messages.loaded);
  const follow = useFollowState(subscribeFollow, getFollow);
  const heights = useRef(new Map<string, number>());
  const [virtualRange, setVirtualRange] = useState({
    start: 0,
    end: 40,
    top: 0,
    bottom: 0,
  });
  useLayoutEffect(() => {
    const root = scroll.current;
    if (!root || showYou) return;
    const update = () => {
      // Newly mounted rows must be measured before changing the window again.
      // Waiting for ResizeObserver lets estimated and actual heights alternate
      // during layout commits, moving the scroll position back and forth.
      for (const node of root.querySelectorAll<HTMLElement>(
        "[data-room-row]",
      )) {
        const height = Math.ceil(node.getBoundingClientRect().height);
        if (height) heights.current.set(node.dataset.roomRow!, height);
      }
      const estimate = (message: (typeof messages.items)[number]) =>
        heights.current.get(message.id) ||
        Math.max(112, Math.ceil((message.text?.length || 0) / 78) * 23 + 82);
      const offsets: number[] = Array.from({
        length: messages.items.length + 1,
      });
      offsets[0] = 0;
      for (let index = 0; index < messages.items.length; index++)
        offsets[index + 1] = offsets[index]! + estimate(messages.items[index]!);
      const top = root.scrollTop;
      const bottom = top + root.clientHeight;
      let start = 0;
      while (start < messages.items.length && offsets[start + 1]! < top)
        start++;
      let end = start;
      while (end < messages.items.length && offsets[end]! < bottom) end++;
      start = Math.max(0, start - 8);
      end = Math.min(messages.items.length, Math.max(start + 1, end + 8));
      const next = {
        start,
        end,
        top: offsets[start]!,
        bottom: offsets.at(-1)! - offsets[end]!,
      };
      setVirtualRange((old) =>
        old.start === next.start &&
        old.end === next.end &&
        old.top === next.top &&
        old.bottom === next.bottom
          ? old
          : next,
      );
    };
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
            {showYou ? "For you" : roomName(state, room!, leadId)}
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
          {!showYou && messages.before != null && (
            <Button
              variant="subtle"
              size="xs"
              onClick={() => {
                setFollow(false);
                void messages.older().catch((error) => notify(String(error)));
              }}
            >
              Earlier messages
            </Button>
          )}
          {!showYou && messages.after != null && (
            <Button
              variant="subtle"
              size="xs"
              onClick={() => {
                setFollow(false);
                void messages.newer().catch((error) => notify(String(error)));
              }}
            >
              Later messages
            </Button>
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
              state={state}
              stateDir={stateDir}
              focusItemId={focusItemId}
              focusRequestId={focusRequestId}
            />
          ) : (
            <>
              <div aria-hidden="true" style={{ height: virtualRange.top }} />
              {messages.items
                .slice(virtualRange.start, virtualRange.end)
                .map((message) => (
                  <div key={message.id} data-room-row={message.id}>
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

type MessageRoom =
  StateCollections["rooms"] extends ReadonlyMap<string, infer T> ? T : never;
function eligibleRooms(
  state: StateCollections | undefined,
  leadId: string | undefined,
): readonly MessageRoom[] {
  const member = (id: string) =>
    id === leadId || state?.agents.get(id)?.rootId === leadId;
  return [...(state?.rooms.values() || [])].filter(
    (room) =>
      leadId &&
      (room.kind === GeneratedApiRoomKind.federated
        ? (room.localMembers ?? room.members ?? []).some((id) => member(id))
        : room.kind === GeneratedApiRoomKind.broadcast
          ? room.rootId === leadId
          : !!room.members?.length &&
            (room.members.every((id) => member(id)) ||
              (!!room.peerTeamId && room.members.some((id) => member(id))))),
  );
}
function roomName(
  state: StateCollections | undefined,
  room: MessageRoom,
  leadId: string | undefined,
) {
  if (room.kind === GeneratedApiRoomKind.federated)
    return room.peerLabel || room.name;
  if (room.kind === GeneratedApiRoomKind.broadcast) return "Team broadcast";
  const peers = (room.members ?? []).filter((id) => id !== leadId);
  return peers.length === 1
    ? state?.agents.get(peers[0]!)?.name || room.name
    : room.name;
}
function personalItems(
  state: StateCollections | undefined,
  requests: readonly RequestModel[],
) {
  return [...(state?.complaints.values() || []), ...requests];
}
function newest<T extends { created?: number | null | undefined }>(
  items: readonly T[],
): T | undefined {
  return items.reduce<T | undefined>(
    (latest, item) =>
      !latest || (item.created || 0) > (latest.created || 0) ? item : latest,
    undefined,
  );
}
function preview(item: ReturnType<typeof personalItems>[number] | undefined) {
  return (
    (item && ("request" in item ? approvalSummary(item) : item.title)) ||
    "Questions, messages and replies"
  );
}
type ListProps = {
  state: StateCollections | undefined;
  seen: ObservableMap<string, number>;
  selected: string;
  leadId: string | undefined;
  scope: string;
  choose(id: string): void;
};
const RoomRow = observer(function RoomRow({
  item,
  state,
  seen,
  selected,
  leadId,
  choose,
}: ListProps & { item?: MessageRoom }) {
  const requests = useRequestsView({ all: true });
  const latest = item
    ? undefined
    : newest(personalItems(state, requests.allRequests));
  const attention = !item && requests.allRequests.some(isPending);
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
          <strong>{item ? roomName(state, item, leadId) : "For you"}</strong>
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
const RoomList = observer(function RoomList({
  rooms,
  ...props
}: ListProps & { rooms: readonly MessageRoom[] }) {
  useMessageRenderProbe("room-list");
  const requests = useRequestsView({ all: true });
  const [query, setQuery] = useState("");
  const [limit, setLimit] = useState(60);
  useEffect(() => {
    setQuery("");
    setLimit(60);
  }, [props.scope]);
  const matches = (text: string) =>
    text.toLowerCase().includes(query.toLowerCase());
  const filtered = rooms
    .filter(
      (item) =>
        !query ||
        matches(
          roomName(props.state, item, props.leadId) +
            " " +
            (item.lastMessage?.text || ""),
        ),
    )
    .sort(
      (a, b) =>
        (b.lastMessage?.created ?? b.updated ?? 0) -
        (a.lastMessage?.created ?? a.updated ?? 0),
    );
  const personal =
    !query ||
    matches(
      "For you " +
        preview(newest(personalItems(props.state, requests.allRequests))),
    );
  return (
    <nav className="team-room-list" aria-label="Message chats">
      <div className="team-room-search">
        <TextInput
          aria-label="Search chats"
          placeholder="Search message rooms"
          leftSection={<Search size={16} />}
          value={query}
          onChange={(event) => {
            setQuery(event.currentTarget.value);
            setLimit(60);
          }}
        />
      </div>
      <div className="team-room-rows">
        {personal && <RoomRow {...props} />}
        {filtered.slice(0, limit).map((item) => (
          <RoomRow key={item.id} item={item} {...props} />
        ))}
        {filtered.length > limit && (
          <Button
            variant="subtle"
            fullWidth
            onClick={() => setLimit((value) => value + 60)}
          >
            More chats
          </Button>
        )}
        {query && !filtered.length && !personal && (
          <p className="team-chat-empty">
            No rooms match “{query}”. Try an agent or team name.
          </p>
        )}
      </div>
    </nav>
  );
});
const PersonalFeed = observer(function PersonalFeed({
  state,
  stateDir,
  focusItemId,
  focusRequestId,
}: {
  state: StateCollections | undefined;
  stateDir: string;
  focusItemId: string | undefined;
  focusRequestId: string | undefined;
}) {
  const area = useConversationActions();
  const { refresh } = area;
  const { notify } = area.notices;
  const requests = useRequestsView({ all: true });
  useMessageRenderProbe("personal-feed");
  const items = personalItems(
    state,
    requests.allRequests.filter(isPending),
  ).sort((a, b) => (a.created || 0) - (b.created || 0));
  return (
    <RequestArea scope={stateDir} refresh={refresh} notify={notify}>
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

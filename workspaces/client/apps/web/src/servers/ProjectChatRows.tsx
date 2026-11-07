import ChatStatus from "../components/agents/ChatStatus";
import { observer } from "mobx-react-lite";
import {
  ChatTitle,
  ChatIndicator,
  ChatServerLine,
} from "../components/chat-row/index.ts";
export {
  ChatServerLine,
  offlineServerDescription,
} from "../components/chat-row/index.ts";
import type { SidebarChatRow } from "../generated/observableResources.ts";
import { keepCompactSidebarChat } from "../components/sidebar/compact.ts";
import { DEFAULT_LOCAL_SERVER_ALIAS } from "./serverAliases";
import "./sidebar-status.css";

export type ProjectChat =
  | (Pick<
      SidebarChatRow,
      "id" | "name" | "activityAt" | "compactPinned" | "provider" | "indicator"
    > & {
      kind: "chat";
      serverId?: string | null;
    })
  | {
      kind: "room";
      id: string;
      name: string;
      unread: boolean;
      serverId?: string | null;
      provider?: never;
      indicator?: never;
    };
export function keepCompactChat(
  chat: ProjectChat,
  now = Date.now() / 1000,
): boolean {
  switch (chat.kind) {
    case "chat":
      return keepCompactSidebarChat(chat, now);
    case "room":
      return chat.unread;
    default: {
      const exhaustive: never = chat;
      return exhaustive;
    }
  }
}

export function compactVisibleChats(
  chats: ProjectChat[],
  compact: boolean,
  threshold: number,
  selected?: { id: string; serverId: string },
) {
  const shouldCompact = compact && chats.length >= threshold;
  return chats.filter(
    (chat) =>
      !shouldCompact ||
      (selected &&
        selected.id === chat.id &&
        selected.serverId === chat.serverId) ||
      keepCompactChat(chat),
  );
}

export function shouldShowOldChatsControl(
  chatCount: number,
  threshold: number,
) {
  return chatCount >= threshold;
}

const ProjectChatRows = observer(function ProjectChatRows({
  chats,
  selected,
  aliases,
  compact,
  hideOldChatsThreshold,
  query,
  open,
  setCompact,
}: {
  chats: ProjectChat[];
  selected?: { id: string; serverId: string };
  aliases: Record<string, string>;
  compact: boolean;
  hideOldChatsThreshold: number;
  query?: string;
  open: (chat: ProjectChat) => void;
  setCompact: (value: boolean) => void;
}) {
  const shouldCompact = compact && chats.length >= hideOldChatsThreshold;
  const visible = query
    ? chats
    : compactVisibleChats(chats, compact, hideOldChatsThreshold, selected);
  const hidden = chats.length - visible.length;
  return (
    <>
      {visible.map((chat) => (
        <ProjectChatButton
          key={JSON.stringify([chat.serverId, chat.id])}
          chat={chat}
          selected={selected}
          aliases={aliases}
          open={open}
        />
      ))}
      {!query &&
        shouldShowOldChatsControl(chats.length, hideOldChatsThreshold) &&
        (shouldCompact ? hidden > 0 : chats.length > 0) && (
          <button
            className="project-show-more"
            onClick={() => setCompact(!compact)}
          >
            {shouldCompact ? `Show old (${hidden})` : "Hide old"}
          </button>
        )}
    </>
  );
});
export default ProjectChatRows;

const ProjectChatButton = observer(function ProjectChatButton({
  chat,
  selected,
  aliases,
  open,
}: {
  chat: ProjectChat;
  selected: { id: string; serverId: string } | undefined;
  aliases: Record<string, string>;
  open: (chat: ProjectChat) => void;
}) {
  return (
    <button
      className="server-chat"
      data-chat={chat.id}
      aria-current={
        selected?.id === chat.id && selected.serverId === chat.serverId
          ? "page"
          : undefined
      }
      onClick={() => open(chat)}
    >
      <span className="row-copy">
        <ChatTitle row={chat} />
        <ChatServerLine
          row={chat}
          alias={
            aliases[chat.serverId || "local"] || DEFAULT_LOCAL_SERVER_ALIAS
          }
        />
      </span>
      {chat.kind === "chat" ? (
        <ChatIndicator row={chat} />
      ) : (
        <ChatStatus
          status={
            chat.unread ? { kind: "unread", label: "Unread result" } : undefined
          }
        />
      )}
    </button>
  );
});

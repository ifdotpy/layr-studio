import {
  useLayoutEffect,
  useMemo,
  useRef,
  type FormEvent,
  type HTMLAttributes,
} from "react";
import {
  ActionIcon,
  Button,
  Menu,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import {
  Archive,
  ArchiveRestore,
  Folder,
  FolderOpen,
  Mail,
  MoreHorizontal,
  Pencil,
  Pin,
  PinOff,
  Trash2,
  WifiOff,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import type { SidebarChatRow } from "../generated/observableResources.ts";
import { ChatOperationalKind, MarkUnreadEligibility } from "../generated/api.ts";
import ChatStatus from "./agents/ChatStatus";
import { offlineServerDescription } from "../servers/ProjectChatRows";
import { reportPromptComposerRender } from "./prompt-composer/renderProbe";
import LayrCopyProgress from "./sidebar/LayrCopyProgress";

type Actions = {
  prepare: () => void;
  open: () => void;
  pin: () => void;
  rename: (event: FormEvent) => void;
  name: (value: string) => void;
  cancelRename: () => void;
  beginRename: () => void;
  unread: () => void;
  move: () => void;
  changeProject: () => void;
  archive: () => void;
  remove: () => void;
};
type Bindings = HTMLAttributes<HTMLElement> & {
  "data-folder-drop"?: string | undefined;
  "data-sidebar-group"?: string | undefined;
  "data-sidebar-id"?: string | undefined;
  "data-drop-edge"?: string | undefined;
};
type Props = {
  disabled?: boolean | undefined;
  row: SidebarChatRow;
  selected: boolean;
  serverAlias: string;
  showServer: boolean;
  pendingRead?: SidebarChatRow["markRead"] | undefined;
  pinned?: boolean | undefined;
  serverOffline?: boolean | undefined;
  renaming: boolean;
  organizing: boolean;
  name: string;
  compact: boolean;
  markingRead: boolean;
  bindings: Bindings;
  actions: Actions;
};

// Commit the current handlers even when the unchanged row view skips its render.
export default function SidebarRow(props: Props) {
  const current = useRef(props);
  useLayoutEffect(() => {
    current.current = props;
  });
  const actions = useMemo<Actions>(
    () => ({
      prepare: () => current.current.actions.prepare(),
      open: () => current.current.actions.open(),
      pin: () => current.current.actions.pin(),
      rename: (event) => current.current.actions.rename(event),
      name: (value) => current.current.actions.name(value),
      cancelRename: () => current.current.actions.cancelRename(),
      beginRename: () => current.current.actions.beginRename(),
      unread: () => current.current.actions.unread(),
      move: () => current.current.actions.move(),
      changeProject: () => current.current.actions.changeProject(),
      archive: () => current.current.actions.archive(),
      remove: () => current.current.actions.remove(),
    }),
    [],
  );
  const events = useMemo<Bindings>(
    () => ({
      onDragStart: (event) => current.current.bindings.onDragStart?.(event),
      onDragOver: (event) => current.current.bindings.onDragOver?.(event),
      onDragLeave: (event) => current.current.bindings.onDragLeave?.(event),
      onDrop: (event) => current.current.bindings.onDrop?.(event),
      onDragEnd: (event) => current.current.bindings.onDragEnd?.(event),
      onKeyDown: (event) => current.current.bindings.onKeyDown?.(event),
    }),
    [],
  );
  const { draggable } = props.bindings;
  const folderDrop = props.bindings["data-folder-drop"];
  const sidebarGroup = props.bindings["data-sidebar-group"];
  const sidebarId = props.bindings["data-sidebar-id"];
  const dropEdge = props.bindings["data-drop-edge"];
  const bindingValues = useMemo<Bindings>(
    () => ({
      draggable,
      "data-folder-drop": folderDrop,
      "data-sidebar-group": sidebarGroup,
      "data-sidebar-id": sidebarId,
      "data-drop-edge": dropEdge,
    }),
    [draggable, folderDrop, sidebarGroup, sidebarId, dropEdge],
  );
  return (
    <SidebarRowView
      row={props.row}
      selected={props.selected}
      serverAlias={props.serverAlias}
      showServer={props.showServer}
      pendingRead={props.pendingRead}
      pinned={props.pinned}
      serverOffline={props.serverOffline}
      renaming={props.renaming}
      organizing={props.organizing}
      name={props.name}
      compact={props.compact}
      markingRead={props.markingRead}
      disabled={props.disabled}
      bindingValues={bindingValues}
      events={events}
      actions={actions}
    />
  );
}

const SidebarRowView = observer(function SidebarRowView({
  row,
  selected,
  serverAlias,
  showServer,
  pendingRead,
  pinned,
  serverOffline = false,
  renaming,
  organizing,
  name,
  compact,
  markingRead,
  disabled = false,
  bindingValues,
  events,
  actions,
}: Omit<Props, "bindings"> & { bindingValues: Bindings; events: Bindings }) {
  reportPromptComposerRender("sidebar-row", row.id);
  const a = row;
  const target = row.markRead;
  const reading = pendingRead && target &&
    pendingRead.threadId === target.threadId && pendingRead.turnId === target.turnId && pendingRead.revision === target.revision;
  const indicator = reading && row.indicator.kind === ChatOperationalKind.unread
    ? { kind: ChatOperationalKind.none, label: "Read" }
    : row.indicator;
  const unreadHelp = markingRead ? "The read status is changing." : markUnreadHelp(row.markUnread);
  const offlineDescription = serverOffline
    ? offlineServerDescription(serverAlias)
    : undefined;
  const chatRow = (
    <UnstyledButton
      {...bindingValues}
      {...events}
      title={row.name ?? undefined}
      aria-description={[
        "Drag onto a team to join it, or onto the project name to leave. Drop a team chat in the center of another lead chat to make it a subagent. Drag to an edge to reorder. Alt + Up or Down also works.",
        offlineDescription,
        indicator?.label,
      ]
        .filter(Boolean)
        .join(" ")}
      className="chat-row"
      data-chat={row.id}
      onPointerEnter={() => actions.prepare()}
      onFocus={() => actions.prepare()}
      onPointerDown={() => actions.prepare()}
      onClick={() => actions.open()}
      aria-current={selected}
    >
      <span className="row-copy">
        <span className="fleet-chat-heading">
          <ChatStatus
            status={indicator}
            provider={a.provider ?? undefined}
            model={a.model ?? undefined}
          />
          <strong>
            {(pinned ?? a.pinned) && <Pin size={11} className="chat-pin" />}
            {row.name ?? ""}
          </strong>
        </span>
        {row.copyProgress && <LayrCopyProgress progress={row.copyProgress} />}
        {(showServer || serverOffline || row.waitingForLead) && (
          <span className="fleet-chat-meta">
            {(showServer || serverOffline) && (
              <span
                className="fleet-server-badge"
                data-offline={serverOffline || undefined}
                title={offlineDescription || serverAlias}
                aria-label={
                  serverOffline ? offlineDescription : `Server ${serverAlias}`
                }
              >
                {serverOffline && <WifiOff size="1em" aria-hidden="true" />}
                {showServer ? serverAlias : "Offline"}
              </span>
            )}
            {row.waitingForLead && (
              <span className="agent-stall" role="status">
                Waiting for lead
              </span>
            )}
          </span>
        )}
      </span>
    </UnstyledButton>
  );
  return (
    <div
      className={`sidebar-row fleet-strip lead-row ${selected ? "selected" : ""}`}
      data-sidebar-item={row.id}
      data-studio-state={indicator?.kind || "none"}
    >
      {chatRow}
      {renaming ? (
        <form className="inline-rename" onSubmit={(e) => actions.rename(e)}>
          <TextInput
            aria-label="Chat name"
            value={name}
            onChange={(e) => actions.name(e.target.value)}
            autoFocus
            maxLength={80}
            onKeyDown={(e) => {
              if (e.key === "Escape") actions.cancelRename();
            }}
          />
          <Button type="submit" aria-label="Save name" disabled={disabled}>
            Save
          </Button>
          <Button onClick={() => actions.cancelRename()}>Cancel</Button>
        </form>
      ) : (
        <Menu position="bottom-end" withinPortal shadow="lg" width={220}>
          <Menu.Target>
            <ActionIcon
              className="row-actions"
              aria-label={`Actions for ${row.name}`}
            >
              <MoreHorizontal size={16} />
            </ActionIcon>
          </Menu.Target>
          <Menu.Dropdown className="fleet-menu">
            <Menu.Item
              disabled={disabled}
              leftSection={<Pencil size={14} />}
              onClick={actions.beginRename}
            >
              Rename
            </Menu.Item>
            <Menu.Divider />
            <Menu.Item
              leftSection={<Mail size={14} />}
              aria-label="Mark unread"
              disabled={
                disabled ||
                a.markUnread !== MarkUnreadEligibility.allowed ||
                markingRead
              }
              onClick={() => actions.unread()}
            >
              Mark unread
              {unreadHelp && (
                <small className="menu-action-help">
                  {unreadHelp}
                </small>
              )}
            </Menu.Item>
            {a && (
              <>
                <Menu.Item
                  disabled={disabled || organizing}
                  leftSection={
                    (pinned ?? a.pinned) ? <PinOff size={14} /> : <Pin size={14} />
                  }
                  onClick={() => actions.pin()}
                >
                  {(pinned ?? a.pinned) ? "Unpin" : "Pin"}
                </Menu.Item>
                <Menu.Divider />
                {a.projectPath && (
                  <Menu.Item
                    disabled={disabled}
                    leftSection={<Folder size={14} />}
                    onClick={actions.move}
                  >
                    Move to folder
                  </Menu.Item>
                )}
                {!compact && (
                  <Menu.Item
                    leftSection={<FolderOpen size={14} />}
                    disabled={disabled || !a.canChangeProject}
                    onClick={() => actions.changeProject()}
                  >
                    Change project directory
                  </Menu.Item>
                )}
                <Menu.Divider />
                <Menu.Item
                  leftSection={
                    a.archived ? (
                      <ArchiveRestore size={14} />
                    ) : (
                      <Archive size={14} />
                    )
                  }
                  disabled={disabled || !a.canArchive}
                  onClick={() => actions.archive()}
                >
                  {a.archived ? "Restore chat" : "Archive"}
                </Menu.Item>
                <Menu.Divider />
              </>
            )}
            <Menu.Item
              disabled={disabled}
              color="red"
              leftSection={<Trash2 size={14} />}
              onClick={() => actions.remove()}
            >
              Delete
            </Menu.Item>
          </Menu.Dropdown>
        </Menu>
      )}
    </div>
  );
});

function markUnreadHelp(eligibility: SidebarChatRow["markUnread"]): string | null {
  switch (eligibility) {
    case MarkUnreadEligibility.allowed: return null;
    case MarkUnreadEligibility.unsupported: return "Read status is unavailable for this chat.";
    case MarkUnreadEligibility.no_completed_result: return "Available after an answer.";
    case MarkUnreadEligibility.already_unread: return "This answer is already unread.";
  }
  return eligibility satisfies never;
}

import { useLayoutEffect, useMemo, useRef, type HTMLAttributes } from "react";
import { observer } from "mobx-react-lite";
import type { SidebarChatRow } from "../generated/observableResources.ts";
import { reportPromptComposerRender } from "./prompt-composer/renderProbe";
import {
  ChatTitle,
  ChatIndicator,
  ChatCopyProgress,
} from "./chat-row/index.ts";
import { ChatRowActionMenu } from "./chat-row/ChatRowActions.tsx";
import type { ChatRowActions as Actions } from "./chat-row/actions.ts";
import { ChatRowShell, ChatRowButton } from "./chat-row/ChatRowShell.tsx";
import { ChatRowMeta } from "./chat-row/ChatRowMeta.tsx";

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
  return (
    <ChatRowShell row={row} selected={selected} pendingRead={pendingRead}>
      <ChatRowButton
        row={row}
        selected={selected}
        pendingRead={pendingRead}
        serverOffline={serverOffline}
        serverAlias={serverAlias}
        bindings={bindingValues}
        events={events}
        actions={actions}
      >
        <span className="row-copy">
          <span className="fleet-chat-heading">
            <ChatIndicator row={row} pendingRead={pendingRead} />
            <ChatTitle row={row} pinned={pinned} />
          </span>
          <ChatCopyProgress row={row} />
          <ChatRowMeta
            row={row}
            showServer={showServer}
            serverOffline={serverOffline}
            serverAlias={serverAlias}
          />
        </span>
      </ChatRowButton>
      <ChatRowActionMenu
        row={row}
        pinned={pinned}
        renaming={renaming}
        name={name}
        disabled={disabled}
        organizing={organizing}
        compact={compact}
        markingRead={markingRead}
        actions={actions}
      />
    </ChatRowShell>
  );
});

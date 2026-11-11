import { observable } from "mobx";
import { requestId } from "studio-claude-bridge/request-id.ts";
import { localSidebarServices } from "../../support/services.tsx";
import type { SidebarAgent } from "../../support/viewModel.ts";
import type { ChatRowActions, SidebarRowActions } from "./actions.ts";

/** Explicit row UI owner for isolated render witnesses. */
export function rowArea(
  commands: () => ChatRowActions,
  props: {
    id?: string;
    selected?: boolean;
    serverAlias?: string;
    showServer?: boolean;
    renaming?: boolean;
    organizing?: boolean;
    name?: string;
    compact?: boolean;
    markingRead?: boolean;
    pendingRead?: SidebarAgent["markRead"] | undefined;
    pinned?: boolean | undefined;
    serverOffline?: boolean;
    disabled?: boolean;
    bindings?: ReturnType<SidebarRowActions["bindings"]>;
  } = {},
): SidebarRowActions {
  const id = props.id || "a";
  return {
    viewKey: requestId(),
    room: () => undefined,
    row: () => undefined,
    commands,
    ui: {
      services: localSidebarServices,
      aliases: observable.map<string, string>(),
      offline: observable.set<string>(),
      sorting: { target: null, destination: null },
      selection: { selectedId: props.selected ? id : null },
      renaming: props.renaming ? id : null,
      renameInPriority: false,
      name: props.name || "",
      organizing: props.organizing ? id : null,
      visibility: { isCompact: props.compact || false },
      read: {
        marking: new Set(props.markingRead ? [id] : []),
        pendingReadTargets: new Map(
          props.pendingRead == null ? [] : [[id, props.pendingRead]],
        ),
      },
      overrides: observable.map<
        string,
        { pinned?: boolean; archived?: boolean }
      >(props.pinned === undefined ? [] : [[id, { pinned: props.pinned }]]),
    },
    disabled: () => props.disabled || false,
    server: () => ({
      alias: props.serverAlias || "Server",
      show: props.showServer || false,
      offline: props.serverOffline || false,
    }),
    bindings: () => props.bindings || {},
  };
}

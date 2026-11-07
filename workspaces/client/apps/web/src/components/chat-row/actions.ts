import {
  useMemo,
  type FormEvent,
  type DragEvent,
  type KeyboardEvent,
  type HTMLAttributes,
} from "react";
import { computed, compareShallow, makeAutoObservable } from "mobx";
import type { SidebarDragTarget } from "../useSidebarOrder.ts";
import { createArea } from "../area.ts";
import type { SidebarChatView, SidebarAgent } from "../sidebar/viewModel.ts";
export type ChatRowActions = {
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

export type SidebarRowBindings = HTMLAttributes<HTMLElement> & {
  "data-sidebar-id"?: string | undefined;
  "data-sidebar-group"?: string | undefined;
  "data-drop-edge"?: string | undefined;
  "data-folder-drop"?: string | undefined;
};

export type SidebarRowActions = {
  commands(row: SidebarChatView, priority: boolean): ChatRowActions;
  ui: {
    readonly dragTarget?: SidebarDragTarget | null;
    readonly dropDestination?: string | null;
    readonly aliases: ReadonlyMap<string, string>;
    readonly offline: ReadonlySet<string>;
    readonly opened: string | null;
    readonly renaming: string | null;
    readonly renameInPriority: boolean;
    readonly name: string;
    readonly organizing: string | null;
    readonly compact: boolean;
    readonly markingRead: ReadonlySet<string>;
    readonly pendingReadTargets?:
      | ReadonlyMap<string, SidebarAgent["markRead"]>
      | undefined;
    readonly overrides: Readonly<
      Record<string, { pinned?: boolean | undefined }>
    >;
  };
  server(row: SidebarChatView): {
    alias: string;
    show: boolean;
    offline: boolean;
  };
  disabled(row: SidebarChatView): boolean;
  bindings(row: SidebarChatView): SidebarRowBindings;
};

export const [SidebarRowActionsProvider, useSidebarRowActions] =
  createArea<SidebarRowActions>("SidebarRowActions");

export function useChatRowActions(row: SidebarChatView, priority = false) {
  const area = useSidebarRowActions();
  return useMemo<ChatRowActions>(
    () => ({
      prepare: () => area.commands(row, priority).prepare(),
      open: () => area.commands(row, priority).open(),
      pin: () => area.commands(row, priority).pin(),
      rename: (event) => area.commands(row, priority).rename(event),
      name: (value) => area.commands(row, priority).name(value),
      cancelRename: () => area.commands(row, priority).cancelRename(),
      beginRename: () => area.commands(row, priority).beginRename(),
      unread: () => area.commands(row, priority).unread(),
      move: () => area.commands(row, priority).move(),
      changeProject: () => area.commands(row, priority).changeProject(),
      archive: () => area.commands(row, priority).archive(),
      remove: () => area.commands(row, priority).remove(),
    }),
    [area, row, priority],
  );
}

/** Computed booleans stop a different row's UI change at this boundary. */
export function useSidebarRowState(row: SidebarChatView, priority = false) {
  const area = useSidebarRowActions();
  return useMemo(
    () =>
      makeAutoObservable(
        {
          get bindings() {
            return {
              ...area.bindings(row),
              onDragStart: (event: DragEvent<HTMLElement>) =>
                area.bindings(row).onDragStart?.(event),
              onDragOver: (event: DragEvent<HTMLElement>) =>
                area.bindings(row).onDragOver?.(event),
              onDragLeave: (event: DragEvent<HTMLElement>) =>
                area.bindings(row).onDragLeave?.(event),
              onDrop: (event: DragEvent<HTMLElement>) =>
                area.bindings(row).onDrop?.(event),
              onDragEnd: (event: DragEvent<HTMLElement>) =>
                area.bindings(row).onDragEnd?.(event),
              onKeyDown: (event: KeyboardEvent<HTMLElement>) =>
                area.bindings(row).onKeyDown?.(event),
            };
          },
          get dropEdge() {
            const target = area.ui.dragTarget;
            return target?.id === row.id && target.group === row.groupKey
              ? target.after
                ? "after"
                : "before"
              : undefined;
          },
          get folderDrop() {
            return area.ui.dropDestination === `convert:${row.id}`
              ? "true"
              : undefined;
          },
          get server() {
            return area.server(row);
          },
          get disabled() {
            return area.disabled(row);
          },
          get selected() {
            return area.ui.opened === row.id;
          },
          get renaming() {
            return (
              area.ui.renaming === row.id &&
              area.ui.renameInPriority === priority
            );
          },
          get name() {
            return area.ui.renaming === row.id ? area.ui.name : "";
          },
          get organizing() {
            return area.ui.organizing === row.id;
          },
          get markingRead() {
            return area.ui.markingRead.has(row.id);
          },
          get pendingRead() {
            return area.ui.pendingReadTargets?.get(row.id);
          },
          get pinned() {
            return area.ui.overrides[row.id]?.pinned;
          },
        },
        { server: computed({ equals: compareShallow }) },
      ),
    [area, row, priority],
  );
}

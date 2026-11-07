import {
  useMemo,
  type DragEvent,
  type KeyboardEvent,
  type HTMLAttributes,
} from "react";
import { computed, compareShallow, makeAutoObservable } from "mobx";
import { createArea } from "../area.ts";
import type { SidebarChatView } from "../sidebar/viewModel.ts";
type RowArea = ReturnType<
  typeof import("../sidebar/sidebarActions.ts").useSidebarActions
>["rows"];
export type ChatRowActions = ReturnType<RowArea["commands"]>;
export type SidebarRowActions = Omit<RowArea, "ui" | "bindings"> & {
  bindings(row: SidebarChatView): HTMLAttributes<HTMLElement> & {
    "data-sidebar-id"?: string;
    "data-sidebar-group"?: string;
  };
  ui: Omit<RowArea["ui"], "services" | "aliases" | "offline" | "overrides"> & {
    overrides: ReadonlyMap<
      string,
      ReturnType<RowArea["ui"]["overrides"]["get"]>
    >;
  };
};

export const [SidebarRowActionsProvider, useSidebarRowActions] =
  createArea<SidebarRowActions>("SidebarRowActions");

/** Computed booleans stop a different row's UI change at this boundary. */
export function useSidebarRowState(row: SidebarChatView, priority = false) {
  const area = useSidebarRowActions();
  return useMemo(
    () =>
      makeAutoObservable(
        {
          row,
          actions: area.commands(row, priority),
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
            return area.ui.overrides.get(row.id)?.pinned;
          },
        },
        {
          row: false,
          actions: false,
          server: computed({ equals: compareShallow }),
        },
      ),
    [area, row, priority],
  );
}

export type SidebarRowState = ReturnType<typeof useSidebarRowState>;

export type ChatRowProps = { state: SidebarRowState };

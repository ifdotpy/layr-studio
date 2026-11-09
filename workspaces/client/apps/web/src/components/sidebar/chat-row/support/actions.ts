import { useMemo, type DragEvent, type KeyboardEvent } from "react";
import { computed, compareShallow, makeAutoObservable, untracked } from "mobx";
import { createArea } from "../../../../app/area.ts";
import type { SidebarChatView } from "../../support/viewModel.ts";
type RowArea = import("../../support/sidebarActions.ts").SidebarActions["rows"];
export type ChatRowActions = ReturnType<RowArea["commands"]>;
export type SidebarRowActions = RowArea;

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
            const group = row.groupKey;
            return {
              ...untracked(() => area.bindings(row)),
              "data-sidebar-group": group,
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
            const target = area.ui.sorting.target;
            return target?.id === row.id && target.group === row.groupKey
              ? target.after
                ? "after"
                : "before"
              : undefined;
          },
          get server() {
            return area.server(row);
          },
          get disabled() {
            return area.disabled(row);
          },
          get selected() {
            return area.ui.selection.selectedId === row.id;
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
            return area.ui.read.marking.has(row.id);
          },
          get pendingRead() {
            return area.ui.read.pendingReadTargets?.get(row.id);
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

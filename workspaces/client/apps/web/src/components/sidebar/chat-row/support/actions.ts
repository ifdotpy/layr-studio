import { compareShallow, computed, makeAutoObservable, untracked } from "mobx";
import type { DragEvent, KeyboardEvent } from "react";
import { createArea } from "../../../../app/area.ts";
import type { SidebarChatView } from "../../support/viewModel.ts";
type RowArea = import("../../support/sidebarActions.ts").SidebarActions["rows"];
export type ChatRowActions = ReturnType<RowArea["commands"]>;
export type SidebarRowActions = RowArea;

export const [SidebarRowActionsProvider, useSidebarRowActions] =
  createArea<SidebarRowActions>("SidebarRowActions");

export function createSidebarRowState(
  area: SidebarRowActions,
  row: SidebarChatView,
  priority: boolean,
) {
  return makeAutoObservable(
    {
      get row() {
        return area.row(row.id) ?? row;
      },
      get actions() {
        return area.commands(this.row, priority);
      },
      get bindings() {
        const group = this.row.groupKey;
        return {
          ...untracked(() => area.bindings(this.row)),
          "data-sidebar-group": group,
          onDragStart: (event: DragEvent<HTMLElement>) =>
            area.bindings(this.row).onDragStart?.(event),
          onDragOver: (event: DragEvent<HTMLElement>) =>
            area.bindings(this.row).onDragOver?.(event),
          onDragLeave: (event: DragEvent<HTMLElement>) =>
            area.bindings(this.row).onDragLeave?.(event),
          onDrop: (event: DragEvent<HTMLElement>) =>
            area.bindings(this.row).onDrop?.(event),
          onDragEnd: (event: DragEvent<HTMLElement>) =>
            area.bindings(this.row).onDragEnd?.(event),
          onKeyDown: (event: KeyboardEvent<HTMLElement>) =>
            area.bindings(this.row).onKeyDown?.(event),
        };
      },
      get dropEdge() {
        const target = area.ui.sorting.target;
        return target?.id === this.row.id && target.group === this.row.groupKey
          ? target.after
            ? "after"
            : "before"
          : undefined;
      },
      get server() {
        return area.server(this.row);
      },
      get disabled() {
        return area.disabled(this.row);
      },
      get selected() {
        return area.ui.selection.selectedId === this.row.id;
      },
      get renaming() {
        return (
          area.ui.renaming === this.row.id &&
          area.ui.renameInPriority === priority
        );
      },
      get name() {
        return area.ui.renaming === this.row.id ? area.ui.name : "";
      },
      get organizing() {
        return area.ui.organizing === this.row.id;
      },
      get markingRead() {
        return area.ui.read.marking.has(this.row.id);
      },
      get pendingRead() {
        return area.ui.read.pendingReadTargets?.get(this.row.id);
      },
      get pinned() {
        return area.ui.overrides.get(this.row.id)?.pinned;
      },
    },
    {
      server: computed({ equals: compareShallow }),
    },
    { autoBind: true },
  );
}

export type SidebarRowState = ReturnType<typeof createSidebarRowState>;

export type ChatRowProps = { state: SidebarRowState };

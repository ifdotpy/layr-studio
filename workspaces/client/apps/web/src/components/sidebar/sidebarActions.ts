import { computed, runInAction, type IComputedValue } from "mobx";
import type { Project } from "../ProjectOrganization";
import type { PeerTeam } from "../shell/messages/PeerTeams";
import type { SidebarChatView } from "./viewModel.ts";
import type { FormEvent, HTMLAttributes } from "react";
import type { PeerTeamActions } from "../shell/messages/PeerTeams";
import { DEFAULT_LOCAL_SERVER_ALIAS } from "../../servers/serverAliases.ts";
import type { SidebarListOwner } from "../../app/shell/SidebarOwner.ts";

/** Local controls stay with the sidebar; stable command areas read the current handlers. */
export function createSidebarActions(state: SidebarListOwner) {
  const ui: Pick<
    SidebarListOwner,
    | "services"
    | "aliases"
    | "offline"
    | "renaming"
    | "renameInPriority"
    | "name"
    | "organizing"
    | "overrides"
  > & {
    readonly sorting: Pick<
      SidebarListOwner["sorting"],
      "target" | "destination"
    >;
    readonly selection: Pick<SidebarListOwner["selection"], "selectedId">;
    readonly visibility: Pick<SidebarListOwner["visibility"], "isCompact">;
    readonly read: Pick<
      SidebarListOwner["read"],
      "marking" | "pendingReadTargets"
    >;
  } = state;
  const rows = {
    ui,
    commands: (row: SidebarChatView, priority: boolean) => ({
      open: () => state.openChat(row.id),
      pin: () => {
        void state.organize(row.id, {
          pinned: !row.agent.pinned,
        });
      },
      rename: (event: FormEvent) => {
        void state.rename(event, row.id);
      },
      name: (value: string) =>
        runInAction(() => {
          ui.name = value;
        }),
      cancelRename: () =>
        runInAction(() => {
          ui.renaming = null;
        }),
      beginRename: () =>
        runInAction(() => {
          ui.name = row.agent.name ?? "";
          ui.renameInPriority = priority;
          ui.renaming = row.id;
        }),
      unread: () => {
        if (row.agent.markRead)
          void state.run({
            action: "mark-unread",
            id: row.id,
            target: row.agent.markRead,
          });
      },
      move: () => {
        const s = state;
        if (s.requireProjectSupport())
          s.setDialog({
            title: "Move chat",
            path: row.projectPath || "",
            agentId: row.id,
          });
      },
      changeProject: (): void =>
        void state.run({ action: "change-project", id: row.id }),
      archive: () => {
        void state.organize(row.id, {
          archived: !row.agent.archived,
        });
      },
      remove: (): void =>
        void state.run({ action: "remove-chat", id: row.id, room: false }),
    }),
    disabled: (row: SidebarChatView) =>
      !(ui.services.available?.({ kind: "chat", id: row.id }) ?? true),
    bindings: (
      row: SidebarChatView,
    ): HTMLAttributes<HTMLElement> & {
      "data-sidebar-id"?: string;
      "data-sidebar-group"?: string;
    } => state.rowBindings(row),
    server: (row: SidebarChatView) => {
      const services = state.services,
        owner = services.serverForChat(row.id);
      return {
        alias:
          services.alias?.({ kind: "chat", id: row.id }) ||
          ui.aliases.get(owner) ||
          DEFAULT_LOCAL_SERVER_ALIAS,
        show: owner !== ui.services.cache.ownerId,
        offline: !state.canEdit({ kind: "chat", id: row.id }),
      };
    },
  };
  const teams: PeerTeamActions = (() => {
    const candidates = new WeakMap<
      Project,
      Map<PeerTeam | undefined, IComputedValue<readonly SidebarChatView[]>>
    >();
    return {
      get scope() {
        return state.stateDir;
      },
      openRoom: (id) => state.openChat(id),
      refresh: async () => {
        await state.refresh();
      },
      closeForm: () => state.setTeamDialog(null),
      edit: (team, action) =>
        state.setTeamDialog({
          path: team.projectPath || "",
          id: team.id,
          action,
        }),
      toggle: (team) =>
        state.toggleTree(JSON.stringify([team.projectPath, "team", team.id])),
      room: (team) =>
        state.rooms.find((room) => room.radio?.teamId === team.id)?.id,
      disabled: (team) => {
        const s = state;
        return !s.canEdit(
          s.projectTarget(team.projectPath || "", undefined, team.id),
        );
      },
      candidates: (project, team) => {
        let byTeam = candidates.get(project);
        if (!byTeam) candidates.set(project, (byTeam = new Map()));
        const view =
          byTeam.get(team) ??
          computed(() => {
            const s = state;
            return s.model.rows.filter(
              (row) =>
                row.projectPath === project.path &&
                s.services.serverForChat(row.id) ===
                  s.services.owner(
                    s.projectTarget(project.path || "", undefined, team?.id),
                  ).ownerId &&
                !s.peerTeams.some(
                  (other) =>
                    other.id !== team?.id &&
                    (other.members || []).includes(row.id),
                ),
            );
          });
        byTeam.set(team, view);
        return view.get();
      },
    };
  })();
  return { rows, teams };
}

export type SidebarActions = ReturnType<typeof createSidebarActions>;

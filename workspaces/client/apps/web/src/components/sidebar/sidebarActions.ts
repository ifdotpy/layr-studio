import { observable, runInAction } from "mobx";
import { useLayoutEffect, useRef, useState } from "react";
import type { SidebarRowActions } from "../chat-row/actions.ts";
import type { PeerTeamActions } from "../shell/messages/PeerTeams";
import { DEFAULT_LOCAL_SERVER_ALIAS } from "../../servers/serverAliases.ts";
import type { useSidebarController } from "./SidebarState.tsx";

/** Local controls stay with the sidebar; stable command areas read the current handlers. */
export function useSidebarActions(
  state: ReturnType<typeof useSidebarController>,
) {
  const current = useRef(state);
  current.current = state;
  const [ui] = useState(() =>
    observable(
      {
        opened: state.p.opened,
        renaming: state.renaming,
        renameInPriority: state.renameInPriority,
        name: state.name,
        organizing: state.organizing,
        compact: state.compact,
        markingRead: state.p.markingRead,
        pendingReadTargets: state.p.pendingReadTargets,
        overrides: state.overrides,
        aliases: observable.map<string, string>(),
        offline: observable.set<string>(),
        services: state.services,
        dragTarget: state.sorting.target,
        dropDestination: state.sorting.destination,
      },
      {},
      { deep: false },
    ),
  );
  useLayoutEffect(() =>
    runInAction(() => {
      Object.assign(ui, {
        opened: state.p.opened,
        renaming: state.renaming,
        renameInPriority: state.renameInPriority,
        name: state.name,
        organizing: state.organizing,
        compact: state.compact,
        markingRead: state.p.markingRead,
        pendingReadTargets: state.p.pendingReadTargets,
        overrides: state.overrides,
        services: state.services,
        dragTarget: state.sorting.target,
        dropDestination: state.sorting.destination,
      });
      ui.aliases.replace(state.aliases);
      ui.offline.replace([...(state.p.offlineServers || [])]);
    }),
  );
  const [rows] = useState<SidebarRowActions>(() => ({
    ui,
    commands: (row, priority) => current.current.rowActions(row, priority),
    disabled: (row) =>
      !(ui.services.available?.({ kind: "chat", id: row.id }) ?? true),
    bindings: (row) => current.current.rowBindings(row),
    server: (row) => {
      const services = current.current.services,
        owner = services.serverForChat(row.id);
      return {
        alias:
          services.alias?.({ kind: "chat", id: row.id }) ||
          ui.aliases.get(owner) ||
          DEFAULT_LOCAL_SERVER_ALIAS,
        show: owner !== ui.services.cache.ownerId,
        offline: ui.offline.has(owner),
      };
    },
  }));
  const [teams] = useState<PeerTeamActions>(() => ({
    get scope() {
      return current.current.stateDir;
    },
    openRoom: (id) => current.current.openChat(id),
    refresh: async () => {
      await current.current.commands.refresh?.();
    },
    closeForm: () => current.current.setTeamDialog(null),
    edit: (team, action) =>
      current.current.setTeamDialog({
        path: team.projectPath || "",
        id: team.id,
        action,
      }),
    toggle: (team) =>
      current.current.toggleTree(
        JSON.stringify([team.projectPath, "team", team.id]),
      ),
    room: (team) =>
      current.current.runtime?.rooms?.find(
        (room) => room.radio?.teamId === team.id,
      )?.id,
    disabled: (team) => {
      const s = current.current;
      return !s.canEdit(
        s.projectTarget(team.projectPath || "", undefined, team.id),
      );
    },
    candidates: (project, team) => {
      const s = current.current;
      return s.model.rows.filter(
        (row) =>
          row.projectPath === project.path &&
          s.services.serverForChat(row.id) ===
            s.services.owner(
              s.projectTarget(project.path || "", undefined, team?.id),
            ).ownerId &&
          !s.peerTeams.some(
            (other) =>
              other.id !== team?.id && (other.members || []).includes(row.id),
          ),
      );
    },
  }));
  return { rows, teams };
}

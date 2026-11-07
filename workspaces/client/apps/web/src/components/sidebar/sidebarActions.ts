import { runInAction } from "mobx";
import { useRef, useState } from "react";
import type { SidebarChatView } from "./viewModel.ts";
import type { FormEvent } from "react";
import type { PeerTeamActions } from "../shell/messages/PeerTeams";
import { DEFAULT_LOCAL_SERVER_ALIAS } from "../../servers/serverAliases.ts";
import type { useSidebarController } from "./SidebarState.tsx";

/** Local controls stay with the sidebar; stable command areas read the current handlers. */
export function useSidebarActions(
  state: ReturnType<typeof useSidebarController>,
) {
  const current = useRef(state);
  current.current = state;
  const ui = state.ui;
  const [rows] = useState(() => ({
    ui,
    commands: (row: SidebarChatView, priority: boolean) => ({
      prepare: () => current.current.commands.prepareChat?.(row.id),
      open: () => current.current.openChat(row.id),
      pin: () => {
        void current.current.organize(row.id, { pinned: !row.agent.pinned });
      },
      rename: (event: FormEvent) => {
        void current.current.rename(event, row.id);
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
          current.current.commands.markUnread(row.id, {
            ...row.agent.markRead,
          });
      },
      move: () => {
        const s = current.current;
        if (s.requireProjectSupport())
          s.setDialog({
            title: "Move chat",
            path: row.projectPath || "",
            agentId: row.id,
          });
      },
      changeProject: () => current.current.commands.changeProject(row.id),
      archive: () => {
        void current.current.organize(row.id, {
          archived: !row.agent.archived,
        });
      },
      remove: () => current.current.commands.remove(row.id, false),
    }),
    disabled: (row: SidebarChatView) =>
      !(ui.services.available?.({ kind: "chat", id: row.id }) ?? true),
    bindings: (row: SidebarChatView) => current.current.rowBindings(row),
    server: (row: SidebarChatView) => {
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

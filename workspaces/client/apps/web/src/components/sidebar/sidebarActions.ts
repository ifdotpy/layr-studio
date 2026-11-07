import { useSidebarCommands } from "./commands.ts";
import { computed, runInAction, type IComputedValue } from "mobx";
import type { Project } from "../ProjectOrganization";
import type { PeerTeam } from "../shell/messages/PeerTeams";
import { useRef, useState } from "react";
import type { SidebarChatView } from "./viewModel.ts";
import type { FormEvent, HTMLAttributes } from "react";
import type { PeerTeamActions } from "../shell/messages/PeerTeams";
import { DEFAULT_LOCAL_SERVER_ALIAS } from "../../servers/serverAliases.ts";
import type { useSidebarController } from "./SidebarState.tsx";

/** Local controls stay with the sidebar; stable command areas read the current handlers. */
export function useSidebarActions(
  state: ReturnType<typeof useSidebarController>,
) {
  const commands = useSidebarCommands();
  const current = useRef({ state, commands });
  current.current = { state, commands };
  const ui = state.ui;
  const [rows] = useState(() => ({
    ui,
    commands: (row: SidebarChatView, priority: boolean) => ({
      prepare: () => current.current.commands.prepareChat?.(row.id),
      open: () => current.current.state.openChat(row.id),
      pin: () => {
        void current.current.state.organize(row.id, {
          pinned: !row.agent.pinned,
        });
      },
      rename: (event: FormEvent) => {
        void current.current.state.rename(event, row.id);
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
        const s = current.current.state;
        if (s.requireProjectSupport())
          s.setDialog({
            title: "Move chat",
            path: row.projectPath || "",
            agentId: row.id,
          });
      },
      changeProject: () => current.current.commands.changeProject(row.id),
      archive: () => {
        void current.current.state.organize(row.id, {
          archived: !row.agent.archived,
        });
      },
      remove: () => current.current.commands.remove(row.id, false),
    }),
    disabled: (row: SidebarChatView) =>
      !(ui.services.available?.({ kind: "chat", id: row.id }) ?? true),
    bindings: (
      row: SidebarChatView,
    ): HTMLAttributes<HTMLElement> & {
      "data-sidebar-id"?: string;
      "data-sidebar-group"?: string;
    } => current.current.state.rowBindings(row),
    server: (row: SidebarChatView) => {
      const services = current.current.state.services,
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
  const [teams] = useState<PeerTeamActions>(() => {
    const candidates = new WeakMap<
      Project,
      Map<PeerTeam | undefined, IComputedValue<readonly SidebarChatView[]>>
    >();
    return {
      get scope() {
        return current.current.state.stateDir;
      },
      openRoom: (id) => current.current.state.openChat(id),
      refresh: async () => {
        await current.current.commands.refresh?.();
      },
      closeForm: () => current.current.state.setTeamDialog(null),
      edit: (team, action) =>
        current.current.state.setTeamDialog({
          path: team.projectPath || "",
          id: team.id,
          action,
        }),
      toggle: (team) =>
        current.current.state.toggleTree(
          JSON.stringify([team.projectPath, "team", team.id]),
        ),
      room: (team) =>
        current.current.state.runtime?.rooms?.find(
          (room) => room.radio?.teamId === team.id,
        )?.id,
      disabled: (team) => {
        const s = current.current.state;
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
            const s = current.current.state;
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
  });
  return { rows, teams };
}

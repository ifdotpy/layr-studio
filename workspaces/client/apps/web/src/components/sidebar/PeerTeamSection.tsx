import { UnstyledButton } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { PeerTeamGroup } from "../shell/messages/PeerTeams";

import { SidebarBackendProvider } from "../sidebar/services";

import { itemGroup } from "../sidebar/catalog";

import SidebarRow from "../SidebarRow.tsx";
import { sidebarRender } from "./renderProbe.ts";
import { useSidebarState, type NavigableProject } from "./SidebarState.tsx";
export const PeerTeamSection = observer(function PeerTeamSection({
  group,
  team,
}: {
  group: NavigableProject;
  team: ReturnType<typeof useSidebarState>["peerTeams"][number];
}) {
  const sidebar = useSidebarState();
  sidebarRender("PeerTeamSection");
  const teamMembers = team.members ?? [];
  const members = group.chats.filter((a) => teamMembers.includes(a.id));
  const selected = sidebar.model.rows.find(
    (a) => a.id === sidebar.selection.selectedId && teamMembers.includes(a.id),
  );
  if (selected && !members.includes(selected)) members.push(selected);
  if (!members.length && (sidebar.query || sidebar.archive)) return null;
  const key = JSON.stringify([group.path, "team", team.id]);
  const closed = !!sidebar.collapsed[key] && !sidebar.query;
  const teamDrop = (
    reorder:
      | ReturnType<typeof sidebar.sorting.bindings>
      | Record<string, never> = {},
  ) =>
    sidebar.sorting.dropBindings(
      `peer-team:${team.id}`,
      (source, event) => {
        const chat = sidebar.findRow(source.id);
        return !!(
          ((!(event.target as Element).closest(
            ".peer-team-toggle, .project-tree-toggle",
          ) &&
            !reorder.draggable) ||
            sidebar.middleDrop(event)) &&
          chat &&
          sidebar.canEdit(
            sidebar.projectTarget(group.path, undefined, team.id),
          ) &&
          sidebar.canEdit({ kind: "chat", id: chat.id }) &&
          sidebar.services.serverForChat(chat.id) ===
            sidebar.services.owner(
              sidebar.projectTarget(group.path, undefined, team.id),
            ).ownerId &&
          source.group === chat.groupKey &&
          chat.projectPath === group.path &&
          !teamMembers.includes(chat.id) &&
          !sidebar.teamMove.blocked() &&
          sidebar.organizing === null
        );
      },
      ({ id }) => sidebar.teamMove.move(group.project, id, team.id),
      reorder,
    );
  return (
    <SidebarBackendProvider
      key={team.id}
      value={sidebar.services.owner(
        sidebar.projectTarget(group.path, undefined, team.id),
      )}
    >
      <PeerTeamGroup
        itemId={sidebar.teamKey(group.path, team.id)}
        team={team}
        serverAlias={
          sidebar.services.alias &&
          sidebar.services.owner(
            sidebar.projectTarget(group.path, undefined, team.id),
          ).ownerId !==
            sidebar.services.owner(sidebar.projectTarget(group.path)).ownerId
            ? sidebar.services.alias(
                sidebar.projectTarget(group.path, undefined, team.id),
              )
            : undefined
        }
        serverOffline={
          sidebar.offline?.has(
            sidebar.services.owner(
              sidebar.projectTarget(group.path, undefined, team.id),
            ).ownerId,
          ) ?? false
        }
        reorder={teamDrop(
          sidebar.sorting.bindings(
            itemGroup(group.path),
            sidebar.teamKey(group.path, team.id),
            sidebar.itemIds(group),
          ),
        )}
        drop={teamDrop()}
        closed={closed}
      >
        <div className="peer-team-chats">
          {(sidebar.rooms || [])
            .filter(
              (r) =>
                r.radio?.teamId === team.id &&
                (!closed || r.id === sidebar.selection.selectedId),
            )
            .map((r) => (
              <UnstyledButton
                key={r.id}
                className="peer-team-shared-chat"
                aria-current={
                  r.id === sidebar.selection.selectedId ? "page" : undefined
                }
                onClick={() => sidebar.openChat(r.id)}
              >
                Shared chat
              </UnstyledButton>
            ))}
          {(closed
            ? members.filter((a) => a.id === sidebar.selection.selectedId)
            : members
          ).map((row) => (
            <SidebarRow key={row.id} row={row} />
          ))}
        </div>
      </PeerTeamGroup>
    </SidebarBackendProvider>
  );
});

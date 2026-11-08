import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import { type ProjectFolder } from "../ProjectOrganization";

import { itemGroup } from "../sidebar/catalog";

import SidebarRow from "../SidebarRow.tsx";
import { FolderBranch } from "./FolderBranch.tsx";
import { PeerTeamSection } from "./PeerTeamSection.tsx";
import { sidebarRender } from "./renderProbe.ts";
import { useSidebarState, type NavigableProject } from "./SidebarState.tsx";
export const SidebarProjectChats = observer(function SidebarProjectChats({
  group,
  parent = null,
}: {
  group: NavigableProject;
  parent?: ProjectFolder | null;
}) {
  const sidebar = useSidebarState();
  sidebarRender("SidebarProjectChats");
  const folders = group.folders || [];
  const assigned = group.chats
    .filter((chat) => !sidebar.grouped.has(chat.id))
    .filter((chat) =>
      parent
        ? chat.projectFolder === parent.id
        : !folders.some((folder) => folder.id === chat.projectFolder),
    );
  const chats = [...assigned];
  const children = folders
    .filter((folder) => (folder.parentId || null) === (parent?.id || null))
    .filter((folder) => {
      if (sidebar.query || !sidebar.isCompactProject(group.path)) return true;
      const contains = (id: string, seen = new Set<string>()): boolean => {
        if (seen.has(id)) return false;
        seen.add(id);
        return (
          (!(sidebar.model.groups.get(group.path) || []).some(
            (a) => a.projectFolder === id,
          ) &&
            !folders.some((f) => f.parentId === id)) ||
          group.chats.some(
            (a) => a.projectFolder === id && !sidebar.grouped.has(a.id),
          ) ||
          folders.some((f) => f.parentId === id && contains(f.id, seen))
        );
      };
      return contains(folder.id);
    })
    .sort((a, b) => a.name.localeCompare(b.name));
  return (
    <>
      {[
        ...(!parent
          ? sidebar.peerTeams
              .filter((t) => t.projectPath === group.path)
              .map((t) => ({
                id: sidebar.teamKey(group.path, t.id),
                pinned: false,
                node: <PeerTeamSection group={group} team={t} />,
              }))
          : []),
        ...children.map((folder) => ({
          id: sidebar.folderKey(group.path, folder.id),
          pinned: false,
          node: <FolderBranch group={group} folder={folder} parent={parent} />,
        })),
        ...chats.map((chat) => ({
          id: chat.id,
          pinned: !!chat.agent.pinned,
          node: <SidebarRow row={chat} />,
        })),
      ]
        .sort(
          (a, b) =>
            Number(b.pinned) - Number(a.pinned) ||
            sidebar.sorting.rank(
              a.pinned
                ? chats.find((c) => c.id === a.id)!.groupKey
                : itemGroup(group.path, parent?.id || null),
              a.id,
            ) -
              sidebar.sorting.rank(
                b.pinned
                  ? chats.find((c) => c.id === b.id)!.groupKey
                  : itemGroup(group.path, parent?.id || null),
                b.id,
              ),
        )
        .map((item) => (
          <Fragment key={item.id}>{item.node}</Fragment>
        ))}
      {!chats.length &&
        !children.length &&
        (parent ||
          !sidebar.visibleSharedRooms.some(
            (r) => r.projectPath === group.path,
          )) &&
        (parent ||
          !sidebar.peerTeams.some(
            (team) => team.projectPath === group.path,
          )) && <p className="project-empty">No chats</p>}
    </>
  );
});

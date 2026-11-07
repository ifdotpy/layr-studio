import { ActionIcon, Menu, Tooltip, UnstyledButton } from "@mantine/core";
import { Folder, FolderOpen, MoreHorizontal, Plus } from "lucide-react";
import { observer } from "mobx-react-lite";

import { SidebarProjectStates } from "../sidebar/RowStatus.tsx";

import { CompactChatCount } from "./CompactChatCount.tsx";
import { sidebarRender } from "./renderProbe.ts";
import { useSidebarState, type NavigableProject } from "./SidebarState.tsx";
export const ProjectHeading = observer(function ProjectHeading({
  group,
}: {
  group: NavigableProject;
}) {
  const sidebar = useSidebarState();
  sidebarRender("ProjectHeading");
  const editable = sidebar.canEdit(sidebar.projectTarget(group.path));
  const blocked = sidebar.unavailableReason(sidebar.projectTarget(group.path));
  const closed =
    !!sidebar.collapsed[sidebar.projectTreeKey(group.path)] && !sidebar.query;
  return (
    <div className="project-tree-heading" {...sidebar.folderDrop(group)}>
      <UnstyledButton
        type="button"
        {...sidebar.sorting.bindings(
          "projects",
          group.path,
          sidebar.allProjectGroups.map((item) => item.path),
        )}
        aria-description="Drag to reorder. Alt + Up or Down also works."
        className="project-tree-toggle"
        aria-label={group.name}
        title={sidebar.services.displayPath?.(group.path) || group.path}
        aria-expanded={!closed}
        onClick={() => sidebar.toggleTree(sidebar.projectTreeKey(group.path))}
      >
        {closed ? <Folder size={18} /> : <FolderOpen size={18} />}
        <span className="project-tree-name" title={group.name}>
          {group.name}
        </span>
        <SidebarProjectStates
          rows={sidebar.model.groups.get(group.path) || []}
          archived={sidebar.archive}
        />
      </UnstyledButton>
      {!sidebar.archive && (!sidebar.ui.compact || !!group.path) && (
        <Tooltip label={blocked} disabled={!blocked} withinPortal>
          <ActionIcon
            className="project-tree-action"
            aria-label={`New chat in ${group.name}`}
            {...(blocked ? { "aria-description": blocked } : {})}
            {...(blocked ? { "aria-disabled": true } : {})}
            {...(blocked ? { "data-disabled": true } : {})}
            onClick={() => {
              if (!blocked) sidebar.commands.newChat(group.path || undefined);
            }}
            disabled={sidebar.p.creating}
          >
            <Plus size={15} />
          </ActionIcon>
        </Tooltip>
      )}
      {!!group.path && (
        <Menu withinPortal position="bottom-end">
          <Menu.Target>
            <ActionIcon
              className="project-tree-action"
              aria-label={`Options for project ${group.name}`}
            >
              <MoreHorizontal size={15} />
            </ActionIcon>
          </Menu.Target>
          <Menu.Dropdown className="fleet-menu">
            <Menu.Label>Create</Menu.Label>
            <Menu.Item
              disabled={!editable}
              onClick={() => sidebar.editProject(group, "new")}
            >
              New chat folder
            </Menu.Item>
            {sidebar.commands.newSharedChat && (
              <Menu.Item
                disabled={!editable}
                onClick={() => sidebar.commands.newSharedChat?.(group.path)}
              >
                New shared chat
              </Menu.Item>
            )}
            {sidebar.runtime?.peerTeamsVersion === 1 && (
              <Menu.Item
                disabled={!editable}
                onClick={() =>
                  sidebar.setTeamDialog({
                    path: group.path,
                    action: "save",
                  })
                }
              >
                New team
              </Menu.Item>
            )}
            <Menu.Divider />
            <Menu.Label>View</Menu.Label>
            {(sidebar.viewChatCounts.get(group.path) || 0) >=
              sidebar.hideOldChatsThreshold && (
              <Menu.Item
                onClick={() =>
                  sidebar.setCompactProject(
                    group.path,
                    !sidebar.isCompactProject(group.path),
                  )
                }
              >
                {sidebar.isCompactProject(group.path) ? (
                  <CompactChatCount group={group} />
                ) : (
                  "Hide old"
                )}
              </Menu.Item>
            )}
            <Menu.Divider />
            <Menu.Label>Project settings</Menu.Label>
            <Menu.Item
              disabled={!editable}
              onClick={() => sidebar.editProject(group)}
            >
              Rename project
            </Menu.Item>
            {sidebar.commands.projectFolders && (
              <Menu.Item
                disabled={!editable}
                onClick={() => sidebar.commands.projectFolders?.(group.path)}
              >
                Folders
              </Menu.Item>
            )}
            <Menu.Item
              disabled={!editable}
              onClick={() => sidebar.commands.projectAccount(group.path)}
            >
              Project account
            </Menu.Item>
            {group.registered && sidebar.commands.deleteProject && (
              <Menu.Item
                color="red"
                disabled={!editable}
                onClick={() => sidebar.commands.deleteProject?.(group.path)}
              >
                Delete project…
              </Menu.Item>
            )}
          </Menu.Dropdown>
        </Menu>
      )}
    </div>
  );
});

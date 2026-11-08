import { ActionIcon, Menu, Tooltip, UnstyledButton } from "@mantine/core";
import { Folder, FolderOpen, MoreHorizontal, Plus } from "lucide-react";
import { observer } from "mobx-react-lite";

import { SidebarProjectStates } from "../SidebarAttention/SidebarAttention.tsx";

import { CompactChatCount } from "../CompactChatCount/CompactChatCount.tsx";
import { sidebarRender } from "../../../perf/sidebarRenderProbe.ts";
import {
  useSidebarState,
  type NavigableProject,
} from "../support/SidebarState.tsx";
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
          sidebar.projectPaths,
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
      {!sidebar.archive && (!sidebar.visibility.isCompact || !!group.path) && (
        <Tooltip label={blocked} disabled={!blocked} withinPortal>
          <ActionIcon
            className="project-tree-action"
            aria-label={`New chat in ${group.name}`}
            {...(blocked ? { "aria-description": blocked } : {})}
            {...(blocked ? { "aria-disabled": true } : {})}
            {...(blocked ? { "data-disabled": true } : {})}
            onClick={() => {
              if (!blocked)
                void sidebar.run({
                  action: "new-chat",
                  ...(group.path ? { path: group.path } : {}),
                });
            }}
            disabled={sidebar.creation.isCreating}
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
            <Menu.Item
              disabled={!editable}
              onClick={() =>
                void sidebar.run({
                  action: "new-shared-chat",
                  path: group.path,
                })
              }
            >
              New shared chat
            </Menu.Item>
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
              sidebar.preferences.value.hideOldChatsThreshold && (
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
            <Menu.Item
              disabled={!editable}
              onClick={() =>
                void sidebar.run({
                  action: "project-folders",
                  projectId: group.path,
                })
              }
            >
              Folders
            </Menu.Item>
            <Menu.Item
              disabled={!editable}
              onClick={() =>
                void sidebar.run({
                  action: "project-account",
                  path: group.path,
                })
              }
            >
              Project account
            </Menu.Item>
            {group.registered && (
              <Menu.Item
                color="red"
                disabled={!editable}
                onClick={() =>
                  void sidebar.run({
                    action: "delete-project",
                    projectId: group.path,
                  })
                }
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

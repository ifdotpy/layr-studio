import { ActionIcon, Menu, UnstyledButton } from "@mantine/core";
import { Folder, FolderOpen, MoreHorizontal, Plus } from "lucide-react";
import { observer } from "mobx-react-lite";
import { errorText } from "../../api";
import { type ProjectFolder } from "../ProjectOrganization";

import { ChatServerLine } from "../chat-row/ChatServerLine.tsx";
import { itemGroup } from "../sidebar/catalog";

import { sidebarRender } from "./renderProbe.ts";
import { SidebarProjectChats } from "./SidebarProjectChats.tsx";
import { useSidebarState, type NavigableProject } from "./SidebarState.tsx";
export const FolderBranch = observer(function FolderBranch({
  group,
  folder,
  parent,
}: {
  group: NavigableProject;
  folder: ProjectFolder;
  parent: ProjectFolder | null;
}) {
  const sidebar = useSidebarState();
  sidebarRender("FolderBranch");
  const folders = group.folders || [];
  const target = sidebar.projectTarget(group.path, folder.id);
  const backend = sidebar.services.owner(target);
  const editable = sidebar.canEdit(target);
  const id = sidebar.folderKey(group.path, folder.id);
  const closed = sidebar.collapsed[id] && !sidebar.query;
  const occupied =
    folders.some((item) => item.parentId === folder.id) ||
    (sidebar.model.groups.get(group.path) || []).some(
      (chat) => chat.projectFolder === folder.id,
    );
  const siblingIds = sidebar.itemIds(group, parent?.id || null);
  return (
    <section
      className="project-folder"
      data-folder-id={folder.id}
      data-sidebar-item={id}
      key={folder.id}
    >
      <div
        className="project-tree-heading"
        {...sidebar.folderDrop(group, folder)}
      >
        <UnstyledButton
          {...sidebar.folderDrop(
            group,
            folder,
            sidebar.sorting.bindings(
              itemGroup(group.path, parent?.id || null),
              id,
              siblingIds,
            ),
          )}
          aria-description="Drag to reorder. Alt + Up or Down also works."
          className="project-tree-toggle"
          aria-expanded={!closed}
          onClick={() => sidebar.toggleTree(id)}
        >
          {closed ? <Folder size={16} /> : <FolderOpen size={16} />}
          <span>
            {folder.name}
            {sidebar.services.alias &&
              backend.ownerId !==
                sidebar.services.owner(sidebar.projectTarget(group.path))
                  .ownerId && (
                <ChatServerLine
                  alias={sidebar.services.alias(target) || ""}
                  offline={
                    sidebar.p.offlineServers?.has(backend.ownerId) ?? false
                  }
                />
              )}
          </span>
        </UnstyledButton>
        {!sidebar.archive && (
          <ActionIcon
            className="project-tree-action"
            aria-label={`New chat in folder ${folder.name}`}
            disabled={sidebar.p.creating || !editable}
            onClick={() => {
              if (sidebar.requireProjectSupport())
                sidebar.commands.newChat(group.path, folder.id);
            }}
          >
            <Plus size={15} />
          </ActionIcon>
        )}
        <Menu withinPortal position="bottom-end">
          <Menu.Target>
            <ActionIcon
              className="project-tree-action"
              aria-label={`Options for folder ${folder.name}`}
            >
              <MoreHorizontal size={15} />
            </ActionIcon>
          </Menu.Target>
          <Menu.Dropdown className="fleet-menu">
            <Menu.Item
              disabled={!editable}
              onClick={() => sidebar.editProject(group, "new", folder.id)}
            >
              New subfolder
            </Menu.Item>
            <Menu.Item
              disabled={!editable}
              onClick={() => sidebar.editProject(group, folder)}
            >
              Rename folder
            </Menu.Item>
            <Menu.Item
              disabled={occupied || !editable}
              onClick={async () => {
                if (!sidebar.requireProjectSupport()) return;
                try {
                  await backend.post("/api/projects", {
                    action: "remove_folder",
                    path: group.path,
                    folder_id: folder.id,
                    expected_revision: group.organizationRevision || 0,
                  });
                  await sidebar.commands.refresh?.();
                } catch (error) {
                  sidebar.commands.notify?.(errorText(error));
                }
              }}
            >
              Remove empty folder
            </Menu.Item>
          </Menu.Dropdown>
        </Menu>
      </div>
      {!closed && (
        <div className="project-folder-chats">
          <SidebarProjectChats group={group} parent={folder} />
        </div>
      )}
    </section>
  );
});

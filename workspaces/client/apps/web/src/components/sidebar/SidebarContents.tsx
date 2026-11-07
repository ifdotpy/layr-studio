import { ActionIcon, Menu, TextInput } from "@mantine/core";
import {
  MoreHorizontal,
  Plus,
  Search,
  Settings,
  SquarePen,
  Users,
  X,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { layoutValues } from "../../layout/layout-contract.generated";

import { SidebarFrame } from "../sidebar/SidebarFrame";

import { serverViewId } from "../../servers/environment";
import RemoteProjects from "../sidebar/RemoteProjects";
import { SidebarWarningsTarget } from "../SidebarWarnings";

import { SidebarAttention } from "../sidebar/RowStatus.tsx";

import SidebarRow from "../SidebarRow.tsx";
import { sidebarRender } from "./renderProbe.ts";
import { SidebarOrganizationDialogs } from "./SidebarOrganizationDialogs.tsx";
import { SidebarProjectList } from "./SidebarProjectList.tsx";
import { useSidebarState } from "./SidebarState.tsx";
export const SidebarContents = observer(function SidebarContents() {
  const sidebar = useSidebarState();
  sidebarRender("SidebarContents");
  const content = (
    <>
      <span className="sr-only" role="status">
        {sidebar.sorting.announcement}
      </span>
      <div className="sidebar-header">
        <a
          className="brand"
          href={serverViewId ? location.pathname + location.search : "/"}
        >
          <span className="brand-name">
            <strong>LAYR</strong> <span>Studio</span>
          </span>
        </a>
        <SidebarWarningsTarget />
        <ActionIcon
          className="sidebar-search"
          aria-label="Search chats"
          title="Search chats (Ctrl+K / ⌘K)"
          aria-keyshortcuts="Control+k Meta+k"
          onClick={sidebar.commands.onSearch}
        >
          <Search size={18} />
        </ActionIcon>
        {sidebar.commands.settings && (
          <ActionIcon
            variant="subtle"
            aria-label="Studio settings"
            title="Settings"
            onClick={sidebar.commands.settings}
          >
            <Settings size={18} />
          </ActionIcon>
        )}
        {sidebar.ui.compact && (
          <ActionIcon
            aria-label="Close conversations"
            onClick={sidebar.closeSidebar}
          >
            <X size={20} />
          </ActionIcon>
        )}
      </div>
      {sidebar.p.notice && (
        <p role="alert" className="notice">
          {sidebar.p.notice}
        </p>
      )}
      <TextInput
        id="chat-search"
        type="search"
        placeholder="Filter sidebar"
        aria-label="Filter projects and chats"
        leftSection={<Search size={15} />}
        value={sidebar.query}
        onChange={(e) => {
          sidebar.setQuery(e.target.value);
        }}
      />
      {sidebar.teamMove.feedback}
      <nav id="chat-list" className="chat-scroll" aria-label="Chats">
        <SidebarAttention
          rows={sidebar.model.visibleRows}
          renderRow={(row, priority) => (
            <SidebarRow key={row.id} row={row} priority={priority} />
          )}
        />
        <div className="projects-heading">
          <span className="projects-heading-label">
            {sidebar.archive ? "Archived projects" : "Projects"}
          </span>
          <Menu position="bottom-end" withinPortal>
            <Menu.Target>
              <ActionIcon aria-label="Project list options">
                <MoreHorizontal size={16} />
              </ActionIcon>
            </Menu.Target>
            <Menu.Dropdown className="fleet-menu">
              <Menu.Item onClick={() => sidebar.setArchive(!sidebar.archive)}>
                {sidebar.archive ? "Show active chats" : "Show archived chats"}
              </Menu.Item>
              {/* A narrow sidebar hides these two buttons; the menu keeps them. */}
              {sidebar.commands.newSharedChat && (
                <Menu.Item onClick={() => sidebar.commands.newSharedChat?.()}>
                  New shared chat
                </Menu.Item>
              )}
              <Menu.Item onClick={sidebar.commands.addProject}>
                Add project
              </Menu.Item>
            </Menu.Dropdown>
          </Menu>
          <ActionIcon
            className="sidebar-new-chat"
            aria-label="New chat"
            title="New chat"
            disabled={sidebar.p.creating}
            onClick={() => sidebar.commands.newChat()}
          >
            <SquarePen size={16} />
          </ActionIcon>
          {sidebar.commands.newSharedChat && (
            <ActionIcon
              className="sidebar-shared-chat"
              aria-label="New shared chat"
              title="New shared chat"
              onClick={() => sidebar.commands.newSharedChat?.()}
            >
              <Users size={16} />
            </ActionIcon>
          )}
          <ActionIcon
            className="sidebar-add-project"
            aria-label="Add project"
            onClick={sidebar.commands.addProject}
          >
            <Plus size={18} />
          </ActionIcon>
        </div>
        <SidebarProjectList />
        <RemoteProjects groups={sidebar.remoteGroups} />
        {!sidebar.model.visibleRows.length &&
          !sidebar.projectGroups.length &&
          !sidebar.remoteGroups.length && (
            <p className="notice">
              {sidebar.query ? "No matching chats." : "No chats yet."}
            </p>
          )}
      </nav>
    </>
  );
  return (
    <>
      <SidebarFrame
        scope={`${sidebar.p.services.cache.ownerId}:${sidebar.stateDir}`}
        opened={sidebar.p.mobile}
        {...(sidebar.p.collapsed !== undefined
          ? { collapsed: sidebar.p.collapsed }
          : {})}
        close={sidebar.closeSidebar}
        onCompactChange={sidebar.reportCompact}
        {...(sidebar.commands.settings
          ? { drawerZIndex: layoutValues.layers.navigation }
          : {})}
        onEntered={() => {
          if (
            sidebar.lastOpened.current &&
            document.getElementById("sidebar")?.contains(document.activeElement)
          )
            document
              .querySelector<HTMLElement>(
                `#sidebar [data-chat="${CSS.escape(sidebar.lastOpened.current)}"]`,
              )
              ?.focus();
        }}
      >
        {content}
      </SidebarFrame>
      <SidebarOrganizationDialogs />
    </>
  );
});

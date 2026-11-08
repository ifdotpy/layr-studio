import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { PeerTeamMoveFeedback } from "../../shell/messages/PeerTeamGroup/PeerTeamGroup.tsx";
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
import { layoutValues } from "../../../layout/layout-contract.generated.ts";

import { SidebarFrame } from "../SidebarFrame/SidebarFrame.tsx";

import { serverViewId } from "../../../servers/environment.ts";
import RemoteProjects from "../RemoteProjects/RemoteProjects.tsx";
import { SidebarWarningsTarget } from "../SidebarWarnings/SidebarWarnings.tsx";

import { SidebarAttention } from "../SidebarAttention/SidebarAttention.tsx";

import SidebarRow from "../SidebarRow/SidebarRow.tsx";
import { sidebarRender } from "../../../perf/sidebarRenderProbe.ts";
import { SidebarOrganizationDialogs } from "../SidebarOrganizationDialogs/SidebarOrganizationDialogs.tsx";
import { SidebarProjectList } from "../SidebarProjectList/SidebarProjectList.tsx";
import { useSidebarState } from "../support/SidebarState.tsx";

const renderAttentionRow: Parameters<
  typeof SidebarAttention
>[0]["renderRow"] = (row, priority) => (
  <SidebarRow key={row.id} row={row} priority={priority} />
);

export const SidebarContents = observer(function SidebarContents({
  notice,
}: {
  notice?: string | undefined;
}) {
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
          onClick={() =>
            void sidebar.run({
              action: "search",
              query: "",
              requestId: randomUUID(),
            })
          }
        >
          <Search size={18} />
        </ActionIcon>
        <ActionIcon
          variant="subtle"
          aria-label="Studio settings"
          title="Settings"
          onClick={() => void sidebar.run({ action: "settings" })}
        >
          <Settings size={18} />
        </ActionIcon>
        {sidebar.visibility.isCompact && (
          <ActionIcon
            aria-label="Close conversations"
            onClick={sidebar.closeSidebar}
          >
            <X size={20} />
          </ActionIcon>
        )}
      </div>
      {notice && (
        <p role="alert" className="notice">
          {notice}
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
      <PeerTeamMoveFeedback owner={sidebar.teamMove} />
      <nav id="chat-list" className="chat-scroll" aria-label="Chats">
        <SidebarAttention
          rows={sidebar.model.visibleRows}
          renderRow={renderAttentionRow}
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
              <Menu.Item
                onClick={() => void sidebar.run({ action: "new-shared-chat" })}
              >
                New shared chat
              </Menu.Item>
              <Menu.Item
                onClick={() => void sidebar.run({ action: "add-project" })}
              >
                Add project
              </Menu.Item>
            </Menu.Dropdown>
          </Menu>
          <ActionIcon
            className="sidebar-new-chat"
            aria-label="New chat"
            title="New chat"
            disabled={sidebar.creation.isCreating}
            onClick={() => void sidebar.run({ action: "new-chat" })}
          >
            <SquarePen size={16} />
          </ActionIcon>
          <ActionIcon
            className="sidebar-shared-chat"
            aria-label="New shared chat"
            title="New shared chat"
            onClick={() => void sidebar.run({ action: "new-shared-chat" })}
          >
            <Users size={16} />
          </ActionIcon>
          <ActionIcon
            className="sidebar-add-project"
            aria-label="Add project"
            onClick={() => void sidebar.run({ action: "add-project" })}
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
      <SidebarFrame drawerZIndex={layoutValues.layers.navigation}>
        {content}
      </SidebarFrame>
      <SidebarOrganizationDialogs />
    </>
  );
});

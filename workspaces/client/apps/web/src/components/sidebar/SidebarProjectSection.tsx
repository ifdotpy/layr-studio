import { UnstyledButton } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { PeerTeamForm } from "../shell/messages/PeerTeams";

import { SidebarBackendProvider } from "../sidebar/services";

import { DEFAULT_LOCAL_SERVER_ALIAS } from "../../servers/serverAliases";
import { ChatServerLine } from "../chat-row/ChatServerLine.tsx";

import { CompactChatCount } from "./CompactChatCount.tsx";
import { ProjectHeading } from "./ProjectHeading.tsx";
import { sidebarRender } from "./renderProbe.ts";
import { SidebarProjectChats } from "./SidebarProjectChats.tsx";
import { useSidebarState, type NavigableProject } from "./SidebarState.tsx";
export const SidebarProjectSection = observer(function SidebarProjectSection({
  group,
}: {
  group: NavigableProject;
}) {
  const sidebar = useSidebarState();
  const { teamDialog } = sidebar;
  sidebarRender("SidebarProjectSection");

  const closed =
    !!sidebar.collapsed[sidebar.projectTreeKey(group.path)] && !sidebar.query;
  return (
    <section
      className="sidebar-project"
      data-project-path={group.path}
      key={group.path}
    >
      <ProjectHeading group={group} />
      {teamDialog?.path === group.path &&
        (teamDialog.id &&
        !sidebar.peerTeams.some((t) => t.id === teamDialog.id) ? (
          <p role="alert">
            This team is no longer available.{" "}
            <UnstyledButton onClick={() => sidebar.setTeamDialog(null)}>
              Close
            </UnstyledButton>
          </p>
        ) : (
          <SidebarBackendProvider
            value={sidebar.services.owner(
              sidebar.projectTarget(group.path, undefined, teamDialog.id),
            )}
          >
            <PeerTeamForm
              key={`${teamDialog.path}:${teamDialog.id || "new"}:${teamDialog.action}`}
              project={sidebar.ownedProject(
                group,
                sidebar.projectTarget(group.path, undefined, teamDialog.id),
              )}
              team={sidebar.peerTeams.find((t) => t.id === teamDialog.id)}
              action={teamDialog.action}
            />
          </SidebarBackendProvider>
        ))}
      {!closed && (
        <div className="project-chats">
          {sidebar.visibleSharedRooms
            .filter((r) => r.projectPath === group.path)
            .map((room) => {
              const alias =
                sidebar.services.alias?.({ kind: "chat", id: room.id }) ||
                sidebar.aliases.get(sidebar.services.serverForChat(room.id)) ||
                DEFAULT_LOCAL_SERVER_ALIAS;
              const serverOffline =
                sidebar.offline?.has(sidebar.services.serverForChat(room.id)) ??
                false;
              const chatRow = (
                <UnstyledButton
                  className="chat-row"
                  aria-description={
                    serverOffline
                      ? `${alias} is unreachable. Showing the last known state.`
                      : undefined
                  }
                  aria-current={
                    sidebar.selection.selectedId === room.id
                      ? "page"
                      : undefined
                  }
                  onClick={() => sidebar.openChat(room.id)}
                >
                  <span className="row-copy">
                    <strong>{room.name}</strong>
                    {sidebar.services.serverForChat(room.id) !==
                      sidebar.services.cache.ownerId && (
                      <ChatServerLine
                        provider={
                          sidebar.model.rows
                            .map((row) => row.agent)
                            .find((agent) => room.members?.includes(agent.id))
                            ?.provider || undefined
                        }
                        alias={alias}
                        offline={serverOffline}
                      />
                    )}
                    {serverOffline &&
                      sidebar.services.serverForChat(room.id) ===
                        sidebar.services.cache.ownerId && (
                        <span
                          className="fleet-server-badge"
                          data-offline="true"
                        >
                          Offline
                        </span>
                      )}
                  </span>
                </UnstyledButton>
              );
              return (
                <div
                  key={room.id}
                  className={`sidebar-row lead-row ${sidebar.selection.selectedId === room.id ? "selected" : ""}`}
                >
                  {chatRow}
                </div>
              );
            })}
          <SidebarProjectChats group={group} />
          {!sidebar.query &&
            (sidebar.viewChatCounts.get(group.path) || 0) >=
              sidebar.preferences.value.hideOldChatsThreshold &&
            (sidebar.isCompactProject(group.path)
              ? sidebar.model.visibleRows.filter(
                  (a) => a.projectPath === group.path,
                ).length > group.chats.length
              : sidebar.model.visibleRows.some(
                  (a) => a.projectPath === group.path,
                )) && (
              <UnstyledButton
                className="project-show-all"
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
              </UnstyledButton>
            )}
        </div>
      )}
    </section>
  );
});

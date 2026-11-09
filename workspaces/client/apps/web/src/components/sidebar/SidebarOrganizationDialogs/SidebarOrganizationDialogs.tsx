import { Modal } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { ConvertChatForm } from "../../projects/ConvertChatForm/ConvertChatForm.tsx";
import { MoveChatForm } from "../../projects/MoveChatForm/MoveChatForm.tsx";
import { ProjectNameForm } from "../../projects/ProjectNameForm/ProjectNameForm.tsx";

import { SidebarBackendProvider } from "../support/services.tsx";

import { sidebarRender } from "../../../perf/sidebarRenderProbe.ts";
import { useSidebarState } from "../support/SidebarState.tsx";

export const SidebarOrganizationDialogs = observer(
  function SidebarOrganizationDialogs() {
    const sidebar = useSidebarState();
    const { dialog, conversion } = sidebar;
    sidebarRender("SidebarOrganizationDialogs");
    const dialogProject = dialog
      ? sidebar.groupMap.get(dialog.path)
      : undefined;
    const dialogAgent = dialog?.agentId
      ? sidebar.model.rows.find((item) => item.id === dialog.agentId)
      : undefined;
    const dialogFolder = dialogProject?.folders?.find(
      (item) => item.id === dialog?.folder,
    );
    const target = dialogAgent
      ? { kind: "chat" as const, id: dialogAgent.id }
      : sidebar.projectTarget(
          dialog?.path || "",
          dialog?.folder === "new" ? dialog.parentId : dialog?.folder,
        );
    const dialogAvailable =
      dialogProject &&
      (!dialog?.agentId || dialogAgent) &&
      (!dialog?.folder || dialog.folder === "new" || dialogFolder);
    return (
      <>
        <Modal
          opened={!!conversion}
          onClose={sidebar.closeConversion}
          title="Make chat a subagent"
          closeOnClickOutside={false}
        >
          {conversion &&
            (sidebar.model.rows.some(
              (row) => row.id === conversion.source.id,
            ) &&
            sidebar.model.rows.some(
              (row) => row.id === conversion.target.id,
            ) ? (
              <SidebarBackendProvider
                value={sidebar.services.owner({
                  kind: "chat",
                  id: conversion.source.id,
                })}
              >
                <ConvertChatForm
                  key={`${conversion.source.id}:${conversion.target.id}`}
                  project={conversion.project.project}
                  source={conversion.source}
                  target={conversion.target}
                  scope={sidebar.stateDir}
                  saved={async () => {
                    sidebar.openChat(conversion.target.id);
                    sidebar.setConversion(null);
                  }}
                />
              </SidebarBackendProvider>
            ) : (
              <p>Load the chat server to retry the saved request.</p>
            ))}
        </Modal>
        <Modal
          opened={!!dialog}
          onClose={() => sidebar.setDialog(null)}
          title={dialog?.title}
        >
          {dialog &&
            (dialogAvailable && dialogProject ? (
              <SidebarBackendProvider value={sidebar.services.owner(target)}>
                {dialogAgent ? (
                  <MoveChatForm
                    project={sidebar.ownedProject(dialogProject, target)}
                    agent={dialogAgent}
                    saved={sidebar.savedProject}
                  />
                ) : (
                  <ProjectNameForm
                    project={sidebar.ownedProject(dialogProject, target)}
                    displayPath={sidebar.services.displayPath?.(
                      dialogProject.path,
                      target,
                    )}
                    folder={dialog.folder === "new" ? "new" : dialogFolder}
                    parentId={dialog.parentId}
                    saved={sidebar.savedProject}
                  />
                )}
              </SidebarBackendProvider>
            ) : (
              <p role="alert">
                This project, chat, or folder is no longer available.
              </p>
            ))}
        </Modal>
      </>
    );
  },
);

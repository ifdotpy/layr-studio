import { Modal } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { MoveChatForm } from "../../projects/MoveChatForm/MoveChatForm.tsx";
import { ProjectNameForm } from "../../projects/ProjectNameForm/ProjectNameForm.tsx";

import { SidebarBackendProvider } from "../support/services.tsx";

import { sidebarRender } from "../../../perf/sidebarRenderProbe.ts";
import { useSidebarState } from "../support/SidebarState.tsx";

export const SidebarOrganizationDialogs = observer(
  function SidebarOrganizationDialogs() {
    const sidebar = useSidebarState();
    const { dialog } = sidebar;
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
                    save={sidebar.moveChatSave!}
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
                    save={sidebar.projectNameSave!}
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

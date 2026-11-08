import { useEffect } from "react";
import { Modal } from "@mantine/core";
import { useProjectDelete } from "../../../useProjectDelete.ts";
import { ProjectDeleteDialog } from "../../projects/ProjectDeleteDialog/ProjectDeleteDialog.tsx";
import SharedChatCreate from "../../projects/SharedChatCreate/SharedChatCreate.tsx";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const AppDialogs = observer(function AppDialogs() {
  const a = useServerAreas(),
    dialogs = a.ui;
  const project = useProjectDelete({
    state: a.state,
    metadata: a.scope.services.sync.projectionMetadata,
    workspaceId: a.startup.workspaceId,
    services: a.scope.services.sidebarServices,
    onSuspend: dialogs.setProjectScope,
    onResume: () => dialogs.setProjectScope(new Set()),
    onFinish: a.finishDeletion,
    notify: a.notices.notify,
  });
  useEffect(() => {
    dialogs.openProject = project.open;
    return () => {
      dialogs.openProject = () => {};
    };
  }, [dialogs, project.open]);
  const shared = a.creation.sharedCreate;
  return (
    <>
      <Modal
        opened={!!dialogs.descriptor}
        onClose={dialogs.closeDialog}
        title={dialogs.descriptor?.title}
      >
        {dialogs.descriptor?.body}
      </Modal>
      <ProjectDeleteDialog {...project.dialog} />
      {shared && (
        <Modal
          opened
          onClose={a.creation.closeShared}
          title="New shared chat"
          size="lg"
        >
          <SharedChatCreate
            source={a}
            accounts={a.accounts.data}
            initialPath={a.creation.sharedCreationInitialPath}
            refresh={a.refresh}
            created={a.creation.sharedCreated}
          />
        </Modal>
      )}
    </>
  );
});

import { Modal } from "@mantine/core";
import { ProjectDeleteDialog } from "../../projects/ProjectDeleteDialog/ProjectDeleteDialog.tsx";
import SharedChatCreate from "../../projects/SharedChatCreate/SharedChatCreate.tsx";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const AppDialogs = observer(function AppDialogs() {
  const a = useServerAreas(),
    dialogs = a.ui;
  const project = a.projectDeletion;
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
          {a.accounts ? (
            <SharedChatCreate
              source={a}
              accounts={a.accounts}
              initialPath={a.creation.sharedCreationInitialPath}
            />
          ) : (
            <p role="status">Loading accounts…</p>
          )}
        </Modal>
      )}
    </>
  );
});

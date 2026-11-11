import { Modal } from "@mantine/core";
import { observer } from "mobx-react-lite";
import ProjectServerCards from "../../projects/ProjectServerCards/ProjectServerCards.tsx";

import { ShellState } from "../../../app/shell/ShellState.ts";

export const ShellNewChat = observer(function ShellNewChat({
  shell,
}: {
  shell: ShellState;
}) {
  const newChatDialog = shell.newChatDialog;
  return (
    <Modal
      opened={!!newChatDialog}
      closeOnEscape={false}
      onClose={() => shell.set({ newChatDialog: null })}
      title="New chat"
      aria-label="New chat"
      size="lg"
    >
      {newChatDialog && (
        <ProjectServerCards
          owner={shell.projectSelection!}
          project={newChatDialog.project}
          servers={shell.projectServerChoices}
          onCancel={() => shell.set({ newChatDialog: null })}
          onAdd={shell.addProjectServer}
          onChoose={shell.chooseProjectServer}
        />
      )}
    </Modal>
  );
});

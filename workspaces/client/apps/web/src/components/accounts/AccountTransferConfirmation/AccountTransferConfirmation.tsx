import { observer } from "mobx-react-lite";
import { Button, Modal } from "@mantine/core";
import { layerZIndex } from "../../../theme.ts";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const AccountTransferConfirmation = observer(
  function AccountTransferConfirmation() {
    const ui = useServerAreas().accountsUi,
      target = ui.dialogAccount;
    return (
      <Modal
        opened
        onClose={ui.close}
        title="Transfer this chat"
        zIndex={layerZIndex.modalAbovePopover}
        closeOnClickOutside={!ui.pending}
        closeOnEscape={!ui.pending}
        withCloseButton={!ui.pending}
      >
        <p>
          Transfer this chat to{" "}
          <strong>{target?.email || target?.label}</strong>?
        </p>
        <p>
          Active turns move immediately and continue on the destination.
          Subagents keep their accounts.
        </p>
        {target &&
          ui.dialog?.kind === "transfer" &&
          ui.dialog.sourceProvider !== target.provider && (
            <p>The destination model uses the saved chat context.</p>
          )}
        <p>Applies to this chat only.</p>
        {ui.error && <p role="alert">{ui.error}</p>}
        <Button variant="default" disabled={!!ui.pending} onClick={ui.close}>
          Cancel
        </Button>
        <Button
          loading={!!ui.pending}
          disabled={!target || !!ui.pending}
          onClick={() => void ui.confirmTransfer()}
        >
          Transfer chat
        </Button>
      </Modal>
    );
  },
);

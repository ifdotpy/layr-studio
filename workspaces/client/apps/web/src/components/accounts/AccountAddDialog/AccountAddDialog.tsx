import { observer } from "mobx-react-lite";
import {
  AccountAddForm,
  AccountAddActionsContext,
} from "../AccountAddForm/AccountAddForm.tsx";
import { Modal, TextInput } from "@mantine/core";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { layerZIndex } from "../../../theme.ts";
import CodexSignIn from "../CodexSignIn/CodexSignIn.tsx";
import ClaudeSignIn from "../ClaudeSignIn/ClaudeSignIn.tsx";
import { ProviderKind } from "../../../generated/providerKind.ts";
export type AddAccountIntent = {
  provider?: ProviderKind;
  email?: string | null;
  label?: string;
  serverLabel?: string;
};

export default observer(function AccountAddDialog() {
  const ui = useServerAreas().accountsUi,
    draft = ui.addDraft;
  const serverLabel = ui.addIntent?.serverLabel || "This device",
    scope = ui.areas.scope.services.owner.id;
  const SignIn =
    draft.provider === ProviderKind.Codex ? CodexSignIn : ClaudeSignIn;
  return (
    <>
      <Modal
        opened={!ui.addFlow}
        zIndex={layerZIndex.modalAbovePopover}
        onClose={ui.close}
        title="Add account"
        centered
      >
        <AccountAddActionsContext
          value={{ onCancel: ui.close, onContinue: ui.continueAdd }}
        >
          <AccountAddForm model={draft}>
            <TextInput label="Server" value={serverLabel} readOnly />
          </AccountAddForm>
        </AccountAddActionsContext>
      </Modal>
      {ui.addFlow && (
        <SignIn
          label={draft.label}
          email={draft.email}
          scope={scope}
          serverLabel={serverLabel}
          onClose={ui.close}
        />
      )}
    </>
  );
});

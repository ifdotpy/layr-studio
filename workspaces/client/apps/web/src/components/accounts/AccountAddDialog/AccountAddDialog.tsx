import { observer } from "mobx-react-lite";
import {
  AccountAddForm,
  AccountAddActionsContext,
} from "../AccountAddForm/AccountAddForm.tsx";
import { Modal, Stack, TextInput } from "@mantine/core";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { layerZIndex } from "../../../theme.ts";
import AccountSignIn from "../AccountSignIn/AccountSignIn.tsx";
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
      {ui.addFlow && draft.provider === ProviderKind.Codex && (
        <Modal
          opened
          zIndex={layerZIndex.modalAbovePopover}
          onClose={ui.close}
          title={`Sign in to Codex · ${draft.label || "Codex"}`}
          centered
        >
          <Stack gap="sm">
            <TextInput label="Server" value={serverLabel} readOnly />
            <p>
              Open auth.openai.com/codex/device. Use a private window if your
              browser has another ChatGPT account signed in.
            </p>
            <AccountSignIn
              scope={scope}
              emailHint={draft.email}
              label={draft.label}
            />
          </Stack>
        </Modal>
      )}
      {ui.addFlow && draft.provider === ProviderKind.Claude && (
        <ClaudeSignIn
          label={draft.label || "Claude Code"}
          email={draft.email}
          serverLabel={serverLabel}
          scope={scope}
          opened
          onClose={ui.close}
        />
      )}
    </>
  );
});

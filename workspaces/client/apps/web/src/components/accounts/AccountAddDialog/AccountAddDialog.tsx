import { observer } from "mobx-react-lite";
import { runInAction } from "mobx";
import {
  AccountAddForm,
  AccountAddActionsContext,
  createAccountAddDraft,
} from "../AccountAddForm/AccountAddForm.tsx";
import { Modal, Stack, TextInput } from "@mantine/core";
import { useEffect, useState } from "react";
import { layerZIndex } from "../../../theme.ts";
import type { AccountsController } from "../Accounts/Accounts.tsx";
import AccountSignIn from "../AccountSignIn/AccountSignIn.tsx";
import ClaudeAddSignIn from "../ClaudeAddSignIn/ClaudeAddSignIn.tsx";
import { ProviderKind } from "../../../generated/providerKind.ts";

export type AddAccountIntent = {
  provider?: ProviderKind;
  email?: string | null;
  label?: string;
  serverLabel?: string;
};

export default observer(function AccountAddDialog({
  opened,
  intent,
  state,
  onClose,
}: {
  opened: boolean;
  intent: AddAccountIntent | null;
  state: AccountsController;
  onClose: () => void;
}) {
  const [draft] = useState(createAccountAddDraft);
  const [flow, setFlow] = useState(false);
  useEffect(() => {
    if (!opened) return;
    runInAction(() => {
      draft.provider = intent?.provider || "";
      draft.label = intent?.label || "";
      draft.email = intent?.email || "";
    });
    setFlow(!!intent?.provider);
  }, [intent, opened, draft]);
  const serverLabel = intent?.serverLabel || "This computer";
  const close = () => {
    setFlow(false);
    onClose();
  };
  return (
    <>
      <Modal
        opened={opened && !flow}
        zIndex={layerZIndex.modalAbovePopover}
        onClose={close}
        title="Add account"
        centered
      >
        <AccountAddActionsContext
          value={{ onCancel: close, onContinue: () => setFlow(true) }}
        >
          <AccountAddForm model={draft}>
            <TextInput label="Server" value={serverLabel} readOnly />
          </AccountAddForm>
        </AccountAddActionsContext>
      </Modal>
      {opened && flow && draft.provider === ProviderKind.Codex && (
        <Modal
          opened={opened}
          zIndex={layerZIndex.modalAbovePopover}
          onClose={close}
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
              state={state}
              emailHint={draft.email}
              label={draft.label}
            />
          </Stack>
        </Modal>
      )}
      {opened && flow && draft.provider === ProviderKind.Claude && (
        <ClaudeAddSignIn
          label={draft.label || "Claude Code"}
          email={draft.email}
          serverLabel={serverLabel}
          scope={state.scope || "local"}
          opened={opened}
          onClose={close}
        />
      )}
    </>
  );
});

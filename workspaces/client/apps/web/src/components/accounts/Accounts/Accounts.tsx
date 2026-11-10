import { AccountTransferConfirmation } from "../AccountTransferConfirmation/AccountTransferConfirmation.tsx";
import AccountAddDialog from "../AccountAddDialog/AccountAddDialog.tsx";
import CodexSignIn from "../CodexSignIn/CodexSignIn.tsx";
import ClaudeSignIn from "../ClaudeSignIn/ClaudeSignIn.tsx";
import ClaudeProfile from "../../shell/ClaudeProfile/ClaudeProfile.tsx";
import { ProviderKind } from "../../../generated/providerKind.ts";
import { Button, Group, Modal, Stack, TextInput } from "@mantine/core";
import { ACCOUNT_NAME_MAX_CHARS } from "../../../generated/sharedValues.ts";
import { layerZIndex } from "../../../theme.ts";

import type { AccountsState } from "../support/Accounts.tsx";
import { observer } from "mobx-react-lite";
import { useEffect, useLayoutEffect, useRef } from "react";
import type {
  ServerAccount,
  ServerCommand,
} from "../../../servers/navigation.ts";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

const Accounts = observer(function Accounts({
  externalCommand,
  onExternalCommandHandled,
  onModalOpenChange,
}: {
  state: AccountsState | undefined;
  scope?: string | undefined;
  onError: (message: string) => void;
  onModalOpenChange?: (opened: boolean) => void;
  managerOnly?: boolean;
  externalCommand?: Extract<
    ServerCommand,
    { action: "account-sign-in" | "account-action" }
  > | null;
  onExternalCommandHandled?: () => void;
}) {
  const ui = useServerAreas().accountsUi;
  useEffect(() => {
    ui.receive(externalCommand);
    if (externalCommand && ui.summary && !ui.pending)
      onExternalCommandHandled?.();
  }, [ui, externalCommand, ui.summary, ui.pending]);
  const reporter = useRef(onModalOpenChange);
  useLayoutEffect(() => {
    reporter.current = onModalOpenChange;
    onModalOpenChange?.(ui.dialogOpen);
  }, [onModalOpenChange, ui.dialogOpen]);
  useLayoutEffect(() => () => reporter.current?.(false), []);
  if (!ui.summary) return <p role="status">Loading accounts…</p>;
  return (
    <>
      {ui.error && <p role="alert">{ui.error}</p>}
      <ActiveAccountDialog />
    </>
  );
});
export default Accounts;

/** Serialize the generated summary only after it arrives. */
export function frameAccounts(
  data: Pick<AccountsState, "accounts" | "defaultAccountKey">,
): ServerAccount[];
export function frameAccounts(data: undefined): undefined;
export function frameAccounts(
  data: Pick<AccountsState, "accounts" | "defaultAccountKey"> | undefined,
): ServerAccount[] | undefined;
export function frameAccounts(
  data: Pick<AccountsState, "accounts" | "defaultAccountKey"> | undefined,
): ServerAccount[] | undefined {
  return data?.accounts.map((account) => ({
    accountKey: account.id,
    provider: account.provider,
    email: account.email || null,
    plan: account.plan || null,
    status: account.status,
    label: account.label,
    isDefault: account.id === data.defaultAccountKey,
  }));
}

const AccountRenameDialog = observer(function AccountRenameDialog() {
  const ui = useServerAreas().accountsUi;
  return (
    <Modal
      opened
      onClose={ui.close}
      title={`Rename ${ui.dialogAccount?.email || ui.dialogAccount?.label || "account"}`}
      zIndex={layerZIndex.modalAbovePopover}
      centered
      closeOnEscape={!ui.pending}
      closeOnClickOutside={!ui.pending}
      withCloseButton={!ui.pending}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void ui.saveRename();
        }}
      >
        <Stack gap="sm">
          <TextInput
            label="Name"
            value={ui.renameLabel}
            maxLength={ACCOUNT_NAME_MAX_CHARS}
            disabled={!!ui.pending || !!ui.renameRequest}
            onChange={(event) => ui.setRenameLabel(event.currentTarget.value)}
          />
          <Group justify="flex-end">
            <Button
              variant="default"
              onClick={ui.close}
              disabled={!!ui.pending}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={!ui.canSave} loading={!!ui.pending}>
              Save
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
});

const AccountRemovalDialog = observer(function AccountRemovalDialog() {
  const ui = useServerAreas().accountsUi,
    account = ui.dialogAccount;
  const disconnect = ui.dialog?.kind === "disconnect",
    title = disconnect ? "Disconnect account" : "Delete account";
  const replacement = ui.replacementAccount;
  return (
    <Modal
      opened
      onClose={ui.close}
      title={title}
      zIndex={layerZIndex.modalAbovePopover}
      closeOnClickOutside={!ui.pending}
      closeOnEscape={!ui.pending}
      withCloseButton={!ui.pending}
    >
      {disconnect ? (
        <>
          <h3 className="account-disconnect-identity">
            {account?.email || account?.label}
          </h3>
          <ul className="account-disconnect-effects">
            <li>New chats cannot use this account.</li>
            <li>Existing chats keep it. You can reconnect it later.</li>
            <li>
              Projects with this default need another account before new chats
              can start.
            </li>
          </ul>
        </>
      ) : (
        <>
          <p>
            Delete <strong>{account?.email || account?.label}</strong> from this
            account list?
          </p>
          <p>
            New chats and account choices will no longer use it. Existing chats
            and active work keep their account identity. Native credentials and
            saved history are preserved.
          </p>
        </>
      )}
      {ui.targetsDefault && (
        <p className="account-replacement">
          {replacement
            ? `The application default changes to ${replacement.email || replacement.label}.`
            : disconnect
              ? "Connect another account before disconnecting the application default."
              : "Add another connected account before deleting the application default."}
        </p>
      )}
      <div className="account-confirm-actions">
        <Button variant="default" disabled={!!ui.pending} onClick={ui.close}>
          Cancel
        </Button>
        <Button
          color="red"
          loading={!!ui.pending}
          disabled={!ui.canConfirmRemoval}
          onClick={() => void ui.confirmRemoval()}
        >
          {title}
        </Button>
      </div>
      {ui.error && <p role="alert">{ui.error}</p>}
    </Modal>
  );
});

const ActiveAccountDialog = observer(function ActiveAccountDialog() {
  const ui = useServerAreas().accountsUi;
  const account = ui.dialogAccount,
    kind = ui.dialog?.kind,
    scope = ui.areas.scope.services.owner.id;
  return (
    <>
      {kind === "add" && <AccountAddDialog />}
      {kind === "rename" && <AccountRenameDialog />}
      {(kind === "disconnect" || kind === "delete") && <AccountRemovalDialog />}
      {kind === "configureProfile" && (
        <Modal
          opened
          onClose={ui.close}
          title="Configure Claude"
          centered
          zIndex={layerZIndex.modalAbovePopover}
        >
          {account && <ClaudeProfile account={account} expanded />}
        </Modal>
      )}
      {kind === "transfer" && <AccountTransferConfirmation />}
      {kind === "signIn" &&
        account &&
        (account.provider === ProviderKind.Codex ? (
          <CodexSignIn
            key={account.id}
            account={account}
            scope={scope}
            onClose={ui.close}
            serverLabel={ui.serverLabel}
          />
        ) : (
          <ClaudeSignIn
            key={account.id}
            account={account}
            scope={scope}
            onClose={ui.close}
            serverLabel={ui.serverLabel}
            onReady={async () => {
              if (account.disconnected)
                await ui.areas.scope.services.api.post(
                  "/api/accounts/reconnect",
                  { account_key: account.id },
                );
            }}
          />
        ))}
    </>
  );
});

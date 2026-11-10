import type { Account, AccountsState } from "../support/Accounts.tsx";
import { AccountTransferChip } from "../AccountTransferChip/AccountTransferChip.tsx";
import { AccountTransferConfirmation } from "../AccountTransferConfirmation/AccountTransferConfirmation.tsx";
import { observer } from "mobx-react-lite";
import { ACCOUNT_NAME_MAX_CHARS } from "../../../generated/sharedValues.ts";
import { AccountStatus as GeneratedApiAccountStatus } from "../../../generated/api.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  useServerApi,
  useServerStorage,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import ClaudeProfile from "../../shell/ClaudeProfile/ClaudeProfile.tsx";
import ClaudeSignIn from "../ClaudeSignIn/ClaudeSignIn.tsx";
import CodexSignIn from "../CodexSignIn/CodexSignIn.tsx";
import { Button, Group, Modal, Stack, TextInput } from "@mantine/core";
import { layerZIndex } from "../../../theme.ts";
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { errorText } from "../../../api.ts";
import type { Agent } from "../../../types.ts";

import AccountAddDialog, {
  type AddAccountIntent,
} from "../AccountAddDialog/AccountAddDialog.tsx";
import {
  accountForExternalCommand,
  matchingAccountsForExternalCommand,
  confirmAccountDeleteRequest,
  getOrCreateAccountDeleteRequest,
} from "../support/accountDeleteRequests.ts";
import type {
  ServerAccount,
  ServerCommand,
} from "../../../servers/navigation.ts";
import { ProviderKind } from "../../../generated/providerKind.ts";

const Accounts = observer(function Accounts({
  state,
  scope,
  agent,
  accountKey,
  changeAccount,
  onError,
  onModalOpenChange,
  managerOnly = false,
  renderPicker,
  externalCommand,
  onExternalCommandHandled,
  showTransferStatus = true,
}: {
  state: AccountsState | undefined;
  scope?: string | undefined;
  agent?: Agent;
  accountKey?: string;
  onModalOpenChange?: (opened: boolean) => void;
  managerOnly?: boolean;
  renderPicker?: (
    selectAccount: (key: string) => void,
    disabled: boolean,
  ) => ReactNode;
  changeAccount?: (key: string) => Promise<void>;
  onError: (message: string) => void;
  externalCommand?: Extract<
    ServerCommand,
    { action: "account-sign-in" | "account-action" }
  > | null;
  onExternalCommandHandled?: () => void;
  showTransferStatus?: boolean;
}) {
  const { post } = useServerApi();
  const storage = useServerStorage();

  const [pending, setPending] = useState("");
  const [error, setError] = useState("");
  const [claudeLogin, setClaudeLogin] = useState<Account | null>(null);
  const [codexLogin, setCodexLogin] = useState<Account | null>(null);
  const [transferChoice, setTransferChoice] = useState<{
    target: Account;
    sourceProvider: string;
    agentId: string;
    requestId: string;
  } | null>(null);
  const [disconnectChoice, setDisconnectChoice] = useState<Account | null>(
    null,
  );
  const [deleteChoice, setDeleteChoice] = useState<Account | null>(null);
  const [deleteRequestId, setDeleteRequestId] = useState("");
  const [addOpen, setAddOpen] = useState(false);
  const [addIntent, setAddIntent] = useState<AddAccountIntent | null>(null);
  const [externalServerLabel, setExternalServerLabel] = useState("");
  const [renameChoice, setRenameChoice] = useState<Account | null>(null);
  const [claudeProfileChoice, setClaudeProfileChoice] =
    useState<Account | null>(null);
  const [renameLabel, setRenameLabel] = useState("");
  const handledCommand = useRef<ServerCommand | null>(null);
  const actionLock = useRef(false);
  const childModalOpen =
    addOpen ||
    !!renameChoice ||
    !!claudeProfileChoice ||
    !!transferChoice ||
    !!disconnectChoice ||
    !!deleteChoice ||
    !!claudeLogin ||
    !!codexLogin;
  const modalReporter = useRef(onModalOpenChange);
  useLayoutEffect(() => {
    modalReporter.current = onModalOpenChange;
    onModalOpenChange?.(childModalOpen);
  }, [childModalOpen, onModalOpenChange]);
  useLayoutEffect(() => () => modalReporter.current?.(false), []);
  const accounts = state?.accounts || [];
  const selected =
    accounts.find((a) => a.id === accountKey) ||
    state?.archivedAccounts?.find((a) => a.id === accountKey);
  const accountChange = agent?.accountChange;
  const owner = agent?.isLead ? agent : undefined;
  const teamTransfer = owner?.accountTransferProgress;
  const replacement = accounts.find(
    (account) =>
      account.id !== disconnectChoice?.id &&
      !account.disconnected &&
      account.status === GeneratedApiAccountStatus.ready,
  );
  const disconnectsDefault = disconnectChoice?.id === state?.defaultAccountKey;
  const deletesDefault = deleteChoice?.id === state?.defaultAccountKey;
  const deleteReplacement = accounts.find(
    (account) =>
      account.id !== deleteChoice?.id &&
      !account.disconnected &&
      account.status === GeneratedApiAccountStatus.ready,
  );
  const action = async (key: string, run: () => Promise<unknown>) => {
    if (actionLock.current) return;
    actionLock.current = true;
    setPending(key);
    setError("");
    try {
      await run();
    } catch (e) {
      setError(errorText(e));
      onError(errorText(e));
    } finally {
      actionLock.current = false;
      setPending("");
    }
  };
  useEffect(() => {
    if (
      !state ||
      !externalCommand ||
      handledCommand.current === externalCommand
    )
      return;
    const command = externalCommand;
    handledCommand.current = command;
    setExternalServerLabel(
      command.action === "account-sign-in" ? command.serverLabel : "",
    );
    if (
      command.action === "account-sign-in" &&
      command.forceAdd &&
      !command.accountKey
    ) {
      setAddIntent({
        provider: command.provider,
        email: command.email,
        label: command.label,
        serverLabel: command.serverLabel,
      });
      setAddOpen(true);
      onExternalCommandHandled?.();
      return;
    }
    if (
      command.action === "account-action" &&
      command.operation === "configure-profile"
    ) {
      const account = accountForExternalCommand(state.accounts, command);
      if (account?.provider === ProviderKind.Claude)
        setClaudeProfileChoice(account);
      else if (
        matchingAccountsForExternalCommand(state.accounts, command).length > 1
      )
        setError("Multiple saved accounts match. Select the exact profile.");
      else setError("This Claude account is no longer saved on this server.");
      onExternalCommandHandled?.();
      return;
    }
    const current = state;
    const account = accountForExternalCommand(current.accounts, command);
    if (
      matchingAccountsForExternalCommand(current.accounts, command).length > 1
    ) {
      setError("Multiple saved accounts match. Select the exact profile.");
      onExternalCommandHandled?.();
      return;
    }
    if (command.action === "account-sign-in") {
      if (command.accountKey && !account) {
        setError("This account is no longer saved on this server.");
      } else if (account && (!command.forceAdd || command.accountKey)) {
        const signIn: Record<ProviderKind, (account: Account) => void> = {
          [ProviderKind.Codex]: setCodexLogin,
          [ProviderKind.Claude]: setClaudeLogin,
        };
        signIn[account.provider](account);
      } else {
        setAddIntent({
          provider: command.provider,
          email: command.email,
          label: command.label,
          serverLabel: command.serverLabel,
        });
        setAddOpen(true);
      }
    } else if (!account) {
      setError("This account is no longer saved on this server.");
    } else if (command.operation === "rename") {
      setRenameChoice(account);
      setRenameLabel(account.label);
    } else if (command.operation === "default") {
      void action(`default:${account.id}`, async () => {
        await post("/api/accounts/default", { account_key: account.id });
      });
    } else if (command.operation === "disconnect") {
      setDisconnectChoice(account);
    } else {
      const requestId = getOrCreateAccountDeleteRequest(
        scope,
        account.id,
        storage,
      );
      setDeleteRequestId(requestId);
      setDeleteChoice(account);
    }
    onExternalCommandHandled?.();
  }, [externalCommand, state]);
  const selectAccount = (key: string) => {
    const account = accounts.find((candidate) => candidate.id === key);
    if (
      !account ||
      actionLock.current ||
      pending ||
      !agent?.accountDestinations?.some(
        (destination) =>
          destination.accountKey === account.id &&
          destination.eligibility.kind === "available",
      ) ||
      !accountChange ||
      accountChange.kind === "unavailable"
    )
      return;
    if (account.id === accountKey) return;
    if (accountChange.kind === "transfer" && owner) {
      onModalOpenChange?.(true);
      setTransferChoice({
        target: account,
        sourceProvider: selected?.provider || owner.provider || "",
        agentId: owner.id,
        requestId: randomUUID(),
      });
    } else if (accountChange.kind === "select")
      void action(account.id, async () => {
        await changeAccount?.(account.id);
      });
  };
  if (!state) return <p role="status">Loading accounts…</p>;
  return (
    <>
      {codexLogin && (
        <CodexSignIn
          account={codexLogin}
          scope={scope}
          serverLabel={externalServerLabel || undefined}
          onClose={() => setCodexLogin(null)}
        />
      )}
      {claudeLogin && (
        <ClaudeSignIn
          key={claudeLogin.id}
          account={claudeLogin}
          scope={scope}
          serverLabel={externalServerLabel || undefined}
          onClose={() => setClaudeLogin(null)}
          onReady={async () => {
            if (claudeLogin.disconnected)
              await post("/api/accounts/reconnect", {
                account_key: claudeLogin.id,
              });
          }}
        />
      )}
      {!managerOnly &&
        renderPicker &&
        renderPicker(
          selectAccount,
          !!pending || !accountChange || accountChange.kind === "unavailable",
        )}
      {!managerOnly && renderPicker && showTransferStatus && (
        <AccountTransferChip transfer={teamTransfer ?? undefined} />
      )}
      {error && <p role="alert">{error}</p>}
      <AccountAddDialog
        opened={addOpen}
        intent={addIntent}
        scope={scope}
        onClose={() => setAddOpen(false)}
      />
      <Modal
        opened={!!renameChoice}
        zIndex={layerZIndex.modalAbovePopover}
        onClose={() => setRenameChoice(null)}
        title={`Rename ${renameChoice?.email || renameChoice?.label || "account"}`}
        centered
      >
        <form
          onSubmit={(event) => {
            event.preventDefault();
            if (!renameChoice) return;
            void action(`rename:${renameChoice.id}`, async () => {
              await post("/api/accounts/name", {
                account_key: renameChoice.id,
                label: renameLabel,
                request_id: randomUUID(),
              });
              setRenameChoice(null);
            });
          }}
        >
          <Stack gap="sm">
            <TextInput
              label="Name"
              value={renameLabel}
              maxLength={ACCOUNT_NAME_MAX_CHARS}
              onChange={(event) => setRenameLabel(event.currentTarget.value)}
            />
            <Group justify="flex-end">
              <Button variant="default" onClick={() => setRenameChoice(null)}>
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={!renameLabel.trim() || !!pending}
                loading={pending === `rename:${renameChoice?.id}`}
              >
                Save
              </Button>
            </Group>
          </Stack>
        </form>
      </Modal>
      <Modal
        opened={!!claudeProfileChoice}
        zIndex={layerZIndex.modalAbovePopover}
        onClose={() => setClaudeProfileChoice(null)}
        title="Configure Claude"
        centered
      >
        {claudeProfileChoice && (
          <ClaudeProfile account={claudeProfileChoice} expanded />
        )}
      </Modal>
      <AccountTransferConfirmation
        opened={!!transferChoice}
        target={transferChoice?.target || null}
        sourceProvider={transferChoice?.sourceProvider || ""}
        onClose={() => {
          setTransferChoice(null);
          onModalOpenChange?.(false);
        }}
        onConfirm={async () => {
          if (!transferChoice) return;
          await post("/api/agents/account-transfer", {
            id: transferChoice.agentId,
            account_key: transferChoice.target.id,
            request_id: transferChoice.requestId,
          });
        }}
      />
      <Modal
        opened={!!disconnectChoice}
        zIndex={layerZIndex.modalAbovePopover}
        onClose={() => {
          if (!pending) setDisconnectChoice(null);
        }}
        title="Disconnect account"
        closeOnClickOutside={!pending}
        closeOnEscape={!pending}
        withCloseButton={!pending}
      >
        <h3 className="account-disconnect-identity">
          {disconnectChoice?.email || disconnectChoice?.label}
        </h3>
        <ul className="account-disconnect-effects">
          <li>New chats cannot use this account.</li>
          <li>Existing chats keep it. You can reconnect it later.</li>
          <li>
            Projects with this default need another account before new chats can
            start.
          </li>
        </ul>
        {disconnectsDefault && (
          <p className="account-replacement">
            {replacement
              ? `The application default changes to ${replacement.email || replacement.label}.`
              : "Connect another account before disconnecting the application default."}
          </p>
        )}
        <div className="account-confirm-actions">
          <Button
            variant="default"
            disabled={!!pending}
            onClick={() => setDisconnectChoice(null)}
          >
            Cancel
          </Button>
          <Button
            variant="filled"
            color="red"
            loading={pending === "disconnect"}
            disabled={!!pending || (disconnectsDefault && !replacement)}
            onClick={() => {
              const accountKey = disconnectChoice?.id;
              if (!accountKey) return;
              void action("disconnect", async () => {
                await post("/api/accounts/disconnect", {
                  account_key: accountKey,
                });
                setDisconnectChoice(null);
              });
            }}
          >
            Disconnect account
          </Button>
        </div>
        {error && <p role="alert">{error}</p>}
      </Modal>
      <Modal
        opened={!!deleteChoice}
        zIndex={layerZIndex.modalAbovePopover}
        onClose={() => {
          if (!pending) {
            setDeleteChoice(null);
          }
        }}
        title="Delete account"
        closeOnClickOutside={!pending}
        closeOnEscape={!pending}
        withCloseButton={!pending}
      >
        <p>
          Delete <strong>{deleteChoice?.email || deleteChoice?.label}</strong>{" "}
          from this account list?
        </p>
        <p>
          New chats and account choices will no longer use it. Existing chats
          and active work keep their account identity. Native credentials and
          saved history are preserved.
        </p>
        {deletesDefault && (
          <p>
            {deleteReplacement
              ? `The application default changes to ${deleteReplacement.email || deleteReplacement.label}.`
              : "Add another connected account before deleting the application default."}
          </p>
        )}
        <Button
          variant="default"
          disabled={!!pending}
          onClick={() => {
            setDeleteChoice(null);
          }}
        >
          Cancel
        </Button>
        <Button
          variant="filled"
          color="red"
          loading={pending === "delete"}
          disabled={!!pending || (deletesDefault && !deleteReplacement)}
          onClick={() => {
            if (!deleteChoice || !deleteRequestId) return;
            void action("delete", async () => {
              await confirmAccountDeleteRequest(
                scope,
                deleteChoice.id,
                deleteRequestId,
                (requestId) =>
                  post("/api/accounts/delete", {
                    account_key: deleteChoice.id,
                    request_id: requestId,
                  }),
                storage,
              );
              setDeleteChoice(null);
              setDeleteRequestId("");
            });
          }}
        >
          Delete account
        </Button>
        {error && <p role="alert">{error}</p>}
      </Modal>
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

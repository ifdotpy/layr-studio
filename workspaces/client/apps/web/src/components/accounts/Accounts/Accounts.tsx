import type { Account, AccountsState } from "../support/Accounts.tsx";
import { AccountTransferChip } from "../AccountTransferChip/AccountTransferChip.tsx";
import { AccountCapacity } from "../AccountCapacity/AccountCapacity.tsx";
import { AccountTransferConfirmation } from "../AccountTransferConfirmation/AccountTransferConfirmation.tsx";
import { observer } from "mobx-react-lite";
import {
  makeAutoObservable,
  observable,
  runInAction,
  computedStruct,
  compareStructural,
} from "mobx";
import { resourceData } from "../../../store/runtime.ts";
import { ACCOUNT_NAME_MAX_CHARS } from "../../../generated/sharedValues.ts";
import DirectoryField from "../../projects/DirectoryField/DirectoryField.tsx";
import { AccountStatus as GeneratedApiAccountStatus } from "../../../generated/api.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import ClaudeOrganization from "../ClaudeOrganization/ClaudeOrganization.tsx";
import Disclosure from "../../ui/Disclosure/Disclosure.tsx";
import {
  useServerApi,
  useServerStore,
  useServerStorage,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { accountDisplayName, accountCanStart } from "../../../accountName.ts";
import { ProviderMark } from "../ProviderMark/ProviderMark.tsx";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import ErrorDescription from "../../shell/ErrorDescription/ErrorDescription.tsx";
import ClaudeProfile from "../../shell/ClaudeProfile/ClaudeProfile.tsx";
import AccountStartThrottle from "../AccountStartThrottle/AccountStartThrottle.tsx";
import AccountGroup from "../AccountGroup/AccountGroup.tsx";
import ClaudeSignIn from "../ClaudeSignIn/ClaudeSignIn.tsx";
import CodexSignIn from "../CodexSignIn/CodexSignIn.tsx";
import NativeRuntimeStatus from "../../settings/NativeRuntimeStatus/NativeRuntimeStatus.tsx";
import {
  Accordion,
  Button,
  Group,
  Menu,
  Modal,
  Stack,
  TextInput,
} from "@mantine/core";
import { layerZIndex } from "../../../theme.ts";
import {
  Check,
  Plus,
  RefreshCw,
  Pencil,
  Ellipsis,
  CircleDot,
  LogIn,
  AlertCircle,
} from "lucide-react";
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type SetStateAction,
  type ReactNode,
} from "react";
import { errorText } from "../../../api.ts";
import type { Agent } from "../../../types.ts";

import AccountSignIn from "../AccountSignIn/AccountSignIn.tsx";
import AccountManagerHost from "../AccountManagerHost/AccountManagerHost.tsx";
import AccountAddDialog, {
  type AddAccountIntent,
} from "../AccountAddDialog/AccountAddDialog.tsx";
import {
  accountForExternalCommand,
  matchingAccountsForExternalCommand,
  confirmAccountDeleteRequest,
  getOrCreateAccountDeleteRequest,
} from "../support/accountDeleteRequests.ts";
import type { ServerCommand } from "../../../servers/navigation.ts";
import {
  PROVIDER_KINDS,
  PROVIDER_LABELS,
  ProviderKind,
} from "../../../generated/providerKind.ts";
import {
  closeMenuFromTriggerOnEscape,
  restoreMenuTriggerFocusAfterClose,
  useMenuEscapeSuppression,
} from "../../ui/support/closeMenuFromTriggerOnEscape.ts";

const Accounts = observer(function Accounts({
  state,
  agent,
  accountKey,
  changeAccount,
  onError,
  onModalOpenChange,
  managerOnly = false,
  renderPicker,
  externalCommand,
  onExternalCommandHandled,
  dialogsOnly = false,
  showTransferStatus = true,
}: {
  state: AccountsController;
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
  dialogsOnly?: boolean;
  showTransferStatus?: boolean;
}) {
  const { post } = useServerApi();
  const storage = useServerStorage();

  const [opened, setOpened] = useState(false);
  const [openActionMenuAccountId, setOpenActionMenuAccountId] = useState<
    string | null
  >(null);
  const actionMenuTriggers = useRef(new Map<string, HTMLButtonElement>());
  const { suppressNextEscape, clearSuppression } = useMenuEscapeSuppression(
    openActionMenuAccountId !== null,
  );
  const [pending, setPending] = useState("");
  const [error, setError] = useState("");
  const [home, setHome] = useState("");
  const [adding, setAdding] = useState(false);
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
    (!managerOnly && opened) ||
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
  const accounts = state.data.accounts || [];
  const selected =
    accounts.find((a) => a.id === accountKey) ||
    state.data.archivedAccounts?.find((a) => a.id === accountKey);
  const accountChange = agent?.accountChange;
  const owner = agent?.isLead ? agent : undefined;
  const teamTransfer = owner?.accountTransferProgress;
  const replacement = accounts.find(
    (account) =>
      account.id !== disconnectChoice?.id &&
      !account.disconnected &&
      account.status === GeneratedApiAccountStatus.ready,
  );
  const disconnectsDefault =
    disconnectChoice?.id === state.data.defaultAccountKey;
  const deletesDefault = deleteChoice?.id === state.data.defaultAccountKey;
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
    if (!externalCommand || handledCommand.current === externalCommand) return;
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
      const account = accountForExternalCommand(state.data.accounts, command);
      if (account?.provider === ProviderKind.Claude)
        setClaudeProfileChoice(account);
      else if (
        matchingAccountsForExternalCommand(state.data.accounts, command)
          .length > 1
      )
        setError("Multiple saved accounts match. Select the exact profile.");
      else setError("This Claude account is no longer saved on this server.");
      onExternalCommandHandled?.();
      return;
    }
    const current = state.data;
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
        state.setData(
          await post("/api/accounts/default", { account_key: account.id }),
        );
      });
    } else if (command.operation === "disconnect") {
      setDisconnectChoice(account);
    } else {
      const requestId = getOrCreateAccountDeleteRequest(
        state.scope,
        account.id,
        storage,
      );
      setDeleteRequestId(requestId);
      setDeleteChoice(account);
    }
    onExternalCommandHandled?.();
  }, [externalCommand]);
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
  return (
    <>
      {codexLogin && (
        <CodexSignIn
          account={codexLogin}
          state={state}
          serverLabel={externalServerLabel || undefined}
          onClose={() => setCodexLogin(null)}
        />
      )}
      {claudeLogin && (
        <ClaudeSignIn
          key={claudeLogin.id}
          account={claudeLogin}
          scope={state.scope}
          serverLabel={externalServerLabel || undefined}
          onClose={() => setClaudeLogin(null)}
          onReady={async () => {
            if (claudeLogin.disconnected)
              state.setData(
                await post("/api/accounts/reconnect", {
                  account_key: claudeLogin.id,
                }),
              );
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
      {!dialogsOnly && (
        <AccountManagerHost
          inline={managerOnly}
          opened={opened}
          onClose={() => setOpened(false)}
          title="Accounts"
          closeBlocked={!!disconnectChoice || !!deleteChoice || !!claudeLogin}
        >
          {adding ? (
            <>
              <div className="accounts-add-header">
                <h3>Add account</h3>
                <Button variant="subtle" onClick={() => setAdding(false)}>
                  Back to accounts
                </Button>
              </div>
              <AccountSignIn state={state} />
            </>
          ) : (
            <div className="accounts-setup">
              {managerOnly && <h3>Accounts</h3>}
              {!accounts.length && (
                <p className="accounts-intro">
                  No accounts found. Sign in or find profiles on this computer.
                </p>
              )}
              <div className="accounts-actions">
                <Button
                  variant="filled"
                  onClick={() => {
                    setAddIntent(null);
                    setAddOpen(true);
                  }}
                  leftSection={<Plus size={14} />}
                >
                  Add account
                </Button>
                <Button
                  variant="default"
                  leftSection={<RefreshCw size={14} />}
                  loading={pending === "discover"}
                  disabled={!!pending}
                  onClick={() =>
                    void action("discover", async () => {
                      state.setData(await post("/api/accounts/discover", {}));
                    })
                  }
                >
                  Find existing accounts
                </Button>
              </div>
            </div>
          )}
          {!adding && (
            <>
              <p className="accounts-intro">
                New chats use the application default unless the project has its
                own default. Existing chats keep their account.
              </p>
              <Accordion
                className="accounts-list"
                aria-label="Saved accounts"
                keepMounted={false}
                transitionDuration={0}
                chevronPosition="left"
              >
                {PROVIDER_KINDS.map((provider) => {
                  const providerAccounts = accounts.filter(
                    (account) => account.provider === provider,
                  );
                  return providerAccounts.length ? (
                    <section
                      className="accounts-provider"
                      key={provider}
                      aria-label={`${PROVIDER_LABELS[provider]} accounts`}
                    >
                      <h3>
                        <ProviderMark provider={provider} />
                        {PROVIDER_LABELS[provider]}
                      </h3>
                      <div className="accounts-grid">
                        {providerAccounts.map((account) => (
                          <Accordion.Item
                            value={account.id}
                            className="account-row"
                            key={account.id}
                            data-account={account.id}
                            aria-label={account.email || account.label}
                          >
                            <div className="account-card-heading">
                              <Accordion.Control
                                aria-label={`Limit details for ${account.email || account.label}`}
                              >
                                <span className="account-identity">
                                  <span
                                    role="img"
                                    aria-label={PROVIDER_LABELS[provider]}
                                  >
                                    <ProviderMark provider={provider} />
                                  </span>
                                  <strong
                                    title={account.email || account.label}
                                  >
                                    {accountDisplayName(account)}
                                  </strong>
                                </span>
                              </Accordion.Control>
                              {account.plan && (
                                <span className="account-plan">
                                  {account.plan}
                                </span>
                              )}
                              <span className="account-status-markers">
                                {(account.error ||
                                  account.status !==
                                    GeneratedApiAccountStatus.ready) && (
                                  <span
                                    className="account-strip-state"
                                    data-error={
                                      Boolean(account.error) ||
                                      account.status ===
                                        GeneratedApiAccountStatus.error ||
                                      undefined
                                    }
                                  >
                                    {account.error ||
                                    account.status ===
                                      GeneratedApiAccountStatus.error ? (
                                      <AlertCircle
                                        size={14}
                                        aria-hidden="true"
                                      />
                                    ) : (
                                      <LogIn size={14} aria-hidden="true" />
                                    )}
                                    <span className="account-strip-state-label">
                                      {account.error ||
                                      account.status ===
                                        GeneratedApiAccountStatus.error
                                        ? "Error"
                                        : "Sign in needed"}
                                    </span>
                                  </span>
                                )}
                                {account.id ===
                                  state.data.defaultAccountKey && (
                                  <span
                                    className="account-default"
                                    title="Application default"
                                  >
                                    <Check size={12} />
                                    <span className="sr-only">
                                      Application default
                                    </span>
                                  </span>
                                )}
                                {account.id === accountKey && (
                                  <span
                                    className="account-strip-state"
                                    title="Current account"
                                  >
                                    <CircleDot size={14} aria-hidden="true" />
                                    <span className="sr-only">
                                      Current account
                                    </span>
                                  </span>
                                )}
                              </span>
                              <Menu
                                opened={openActionMenuAccountId === account.id}
                                onChange={(nextOpened) => {
                                  setOpenActionMenuAccountId((current) =>
                                    nextOpened
                                      ? account.id
                                      : current === account.id
                                        ? null
                                        : current,
                                  );
                                  if (!nextOpened)
                                    restoreMenuTriggerFocusAfterClose(
                                      actionMenuTriggers.current.get(
                                        account.id,
                                      ) ?? null,
                                    );
                                }}
                                position="bottom-end"
                                withinPortal={false}
                                withInitialFocusPlaceholder={false}
                              >
                                <Menu.Target>
                                  <ActionButton
                                    ref={(node) => {
                                      if (node)
                                        actionMenuTriggers.current.set(
                                          account.id,
                                          node,
                                        );
                                      else
                                        actionMenuTriggers.current.delete(
                                          account.id,
                                        );
                                    }}
                                    actionRole="quiet"
                                    className="account-card-menu"
                                    onClick={(event) =>
                                      event.currentTarget.focus({
                                        preventScroll: true,
                                      })
                                    }
                                    loading={
                                      pending === `default:${account.id}`
                                    }
                                    data-mantine-stop-propagation={
                                      openActionMenuAccountId === account.id ||
                                      suppressNextEscape
                                        ? "true"
                                        : undefined
                                    }
                                    onKeyDown={(event) =>
                                      closeMenuFromTriggerOnEscape(
                                        event,
                                        openActionMenuAccountId === account.id,
                                        () =>
                                          setOpenActionMenuAccountId(
                                            (current) =>
                                              current === account.id
                                                ? null
                                                : current,
                                          ),
                                        suppressNextEscape,
                                        clearSuppression,
                                      )
                                    }
                                    onBlur={clearSuppression}
                                    aria-label={`Actions for ${account.email || account.label}`}
                                  >
                                    <Ellipsis size={18} />
                                  </ActionButton>
                                </Menu.Target>
                                <Menu.Dropdown data-mantine-stop-propagation="true">
                                  {account.id !==
                                    state.data.defaultAccountKey && (
                                    <Menu.Item
                                      disabled={
                                        !!pending || !accountCanStart(account)
                                      }
                                      aria-label={`Use ${account.email || account.label} by default`}
                                      onClick={() =>
                                        void action(
                                          `default:${account.id}`,
                                          async () => {
                                            state.setData(
                                              await post(
                                                "/api/accounts/default",
                                                {
                                                  account_key: account.id,
                                                },
                                              ),
                                            );
                                          },
                                        )
                                      }
                                    >
                                      Set application default
                                    </Menu.Item>
                                  )}
                                  {account.provider === ProviderKind.Claude && (
                                    <>
                                      <Menu.Item
                                        onClick={() => setClaudeLogin(account)}
                                      >
                                        Sign in again
                                      </Menu.Item>
                                    </>
                                  )}
                                  {account.provider !== ProviderKind.Claude &&
                                    account.accountId && (
                                      <Menu.Item
                                        onClick={() => setCodexLogin(account)}
                                      >
                                        Sign in again
                                      </Menu.Item>
                                    )}
                                  {(state.data.supportsDisconnect ||
                                    state.data.supportsDelete) && (
                                    <Menu.Divider />
                                  )}
                                  {state.data.supportsDisconnect && (
                                    <Menu.Item
                                      color={
                                        account.disconnected ? "gray" : "red"
                                      }
                                      className={
                                        account.disconnected
                                          ? undefined
                                          : "account-destructive-action"
                                      }
                                      disabled={!!pending}
                                      onClick={() => {
                                        if (account.disconnected)
                                          void action(
                                            `reconnect:${account.id}`,
                                            async () => {
                                              state.setData(
                                                await post(
                                                  "/api/accounts/reconnect",
                                                  {
                                                    account_key: account.id,
                                                  },
                                                ),
                                              );
                                            },
                                          );
                                        else setDisconnectChoice(account);
                                      }}
                                    >
                                      {account.disconnected
                                        ? "Reconnect account"
                                        : "Disconnect account"}
                                    </Menu.Item>
                                  )}
                                  {state.data.supportsDelete && (
                                    <Menu.Item
                                      color="red"
                                      className="account-destructive-action"
                                      disabled={!!pending}
                                      onClick={() => {
                                        const requestId =
                                          getOrCreateAccountDeleteRequest(
                                            state.scope,
                                            account.id,
                                            storage,
                                          );
                                        setDeleteRequestId(requestId);
                                        setDeleteChoice(account);
                                      }}
                                    >
                                      Delete account
                                    </Menu.Item>
                                  )}
                                </Menu.Dropdown>
                              </Menu>
                            </div>
                            <AccountCapacity
                              card
                              account={account}
                              facts={
                                <dl className="account-facts">
                                  <dt>Email</dt>
                                  <dd>{account.email || "Unavailable"}</dd>
                                  <dt>Server</dt>
                                  <dd>This computer</dd>
                                  <dt>Default</dt>
                                  <dd>
                                    {account.id === state.data.defaultAccountKey
                                      ? "Application default"
                                      : "No"}
                                  </dd>
                                  <dt>Sign-in</dt>
                                  <dd>
                                    {account.error ||
                                    account.status ===
                                      GeneratedApiAccountStatus.error
                                      ? "Error"
                                      : account.status ===
                                          GeneratedApiAccountStatus.ready
                                        ? "Signed in"
                                        : "Sign in needed"}
                                  </dd>
                                </dl>
                              }
                              extraDetails={
                                <>
                                  <ClaudeOrganization account={account} />
                                  <AccountNameEditor
                                    account={account}
                                    onSaved={state.setData}
                                    disabled={!!pending}
                                  />

                                  {Boolean(account.error) && (
                                    <ErrorDescription
                                      className="account-action-error"
                                      value={account.error}
                                    />
                                  )}
                                  {account.authenticationRecovery && (
                                    <p role="alert">
                                      {account.authenticationRecovery}
                                    </p>
                                  )}
                                  {account.disconnected && (
                                    <p>Hidden from new chats.</p>
                                  )}
                                  {!account.disconnected && (
                                    <AccountStartThrottle
                                      account={account}
                                      onSaved={state.setData}
                                    />
                                  )}
                                  {!account.disconnected && (
                                    <AccountGroup
                                      account={account}
                                      onSaved={state.setData}
                                    />
                                  )}
                                  {account.provider === ProviderKind.Claude && (
                                    <ClaudeProfile
                                      account={account}
                                      onSaved={state.setData}
                                    />
                                  )}
                                </>
                              }
                            />
                          </Accordion.Item>
                        ))}
                      </div>
                    </section>
                  ) : null;
                })}
              </Accordion>
              <NativeRuntimeStatus
                opened={managerOnly || opened}
                accounts={accounts}
              />
            </>
          )}
          {adding && <ClaudeProfile onSaved={state.setData} />}
          {adding && (
            <Disclosure
              className="account-register"
              summary="Add a Codex home directory"
            >
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  void action("register", async () => {
                    state.setData(
                      await post("/api/accounts/register", {
                        home: home.trim(),
                      }),
                    );
                    setHome("");
                  });
                }}
              >
                <DirectoryField
                  label="Codex home"
                  description="Enter or choose the existing .codex directory for this account."
                  placeholder="/path/to/.codex"
                  value={home}
                  onChange={setHome}
                  autoComplete="off"
                />
                <Button
                  type="submit"
                  size="compact-sm"
                  disabled={!home.trim() || !!pending}
                  loading={pending === "register"}
                >
                  Add profile
                </Button>
              </form>
            </Disclosure>
          )}
          {(error || state.error) && (
            <p className="account-action-error" role="alert">
              {error || state.error}
            </p>
          )}
        </AccountManagerHost>
      )}
      <AccountAddDialog
        opened={addOpen}
        intent={addIntent}
        state={state}
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
              state.setData(
                await post("/api/accounts/name", {
                  account_key: renameChoice.id,
                  label: renameLabel,
                  request_id: randomUUID(),
                }),
              );
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
          <ClaudeProfile
            account={claudeProfileChoice}
            expanded
            onSaved={(value) => {
              state.setData(value);
            }}
          />
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
                state.setData(
                  await post("/api/accounts/disconnect", {
                    account_key: accountKey,
                  }),
                );
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
              state.setData(
                await confirmAccountDeleteRequest(
                  state.scope,
                  deleteChoice.id,
                  deleteRequestId,
                  (requestId) =>
                    post("/api/accounts/delete", {
                      account_key: deleteChoice.id,
                      request_id: requestId,
                    }),
                  storage,
                ),
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

export type AccountsController = Pick<
  ReturnType<typeof createAccountsView>,
  "data" | "setData" | "error" | "refresh" | "scope"
>;

/** Full login readbacks remain local; replicated summaries keep their generated identity. */
export function createAccountsView(
  store: ReturnType<typeof useServerStore>,
  get: ReturnType<typeof useServerApi>["get"],
  stateDir?: string,
) {
  const readback = observable.box<AccountsState>(
    {
      accounts: [],
      archivedAccounts: [],
      defaultAccountKey: "default",
      defaultAccountGroups: [],
      logins: [],
      supportsDelete: true,
      supportsDisconnect: true,
    },
    { deep: false, equals: compareStructural },
  );
  const readError = observable.box("");
  let latestRead = 0;
  const view = {
    scope: stateDir,
    get data(): AccountsState {
      const summary = stateDir
        ? resourceData(store.state().current)?.accountsSummary.get("current")
        : undefined;
      return summary
        ? { ...summary, logins: readback.get().logins }
        : readback.get();
    },
    get error() {
      const current = stateDir ? store.state().current : undefined;
      return (
        readError.get() ||
        (current && "error" in current ? errorText(current.error) : "")
      );
    },
    setData(value: SetStateAction<AccountsState>) {
      latestRead++;
      readback.set(typeof value === "function" ? value(view.data) : value);
      readError.set("");
    },
    invalidate() {
      latestRead++;
    },
    async refresh(): Promise<AccountsState | null> {
      const read = ++latestRead;
      try {
        const result = await get("/api/accounts");
        if (read !== latestRead) return null;
        runInAction(() => {
          readback.set(result);
          readError.set("");
        });
        return result;
      } catch (error) {
        if (read === latestRead)
          runInAction(() => readError.set(errorText(error)));
        return null;
      }
    },
  };
  return makeAutoObservable(view, { data: computedStruct }, { autoBind: true });
}

function AccountNameEditor({
  account,
  onSaved,
  disabled,
}: {
  account: Account;
  onSaved: (data: AccountsState) => void;
  disabled: boolean;
}) {
  const { post } = useServerApi();

  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(account.label);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const request = useRef<{ label: string; request_id: string } | null>(null);
  const lock = useRef(false);
  const fallback = account.email?.split("@")[0] || "Account";
  const nameTooLong = [...name.trim()].length > 32;
  const save = async () => {
    if (lock.current || disabled) return;
    const label = name.trim();
    if ([...label].length > 32) {
      setError("Use at most 32 characters.");
      return;
    }
    if (request.current?.label !== label)
      request.current = { label, request_id: randomUUID() };
    lock.current = true;
    setBusy(true);
    setError("");
    try {
      const result = await post("/api/accounts/name", {
        account_key: account.id,
        ...request.current,
      });
      onSaved(result);
      request.current = null;
      setEditing(false);
    } catch (failure) {
      setError(errorText(failure));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  };
  const cancel = () => {
    if (lock.current) return;
    setEditing(false);
    setError("");
  };
  return (
    <div className="account-name">
      {editing ? (
        <form
          className="account-name-form"
          onSubmit={(event) => {
            event.preventDefault();
            void save();
          }}
        >
          <TextInput
            label="Name"
            aria-label={`Name for ${account.email || account.id}`}
            placeholder={fallback}
            value={name}
            onChange={(event) => setName(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                cancel();
              }
            }}
            disabled={busy || disabled}
            autoFocus
            error={
              error || (nameTooLong ? "Use at most 32 characters." : undefined)
            }
          />
          <div className="account-name-actions">
            <ActionButton
              type="submit"
              actionRole="primary"
              loading={busy}
              disabled={disabled || nameTooLong}
            >
              Save
            </ActionButton>
            <ActionButton actionRole="quiet" onClick={cancel} disabled={busy}>
              Cancel
            </ActionButton>
          </div>
        </form>
      ) : (
        <>
          <ActionButton
            actionRole="quiet"
            leftSection={<Pencil size={14} />}
            aria-label={`Edit name for ${account.email || account.id}`}
            disabled={disabled}
            onClick={() => {
              setName(account.label);
              setError("");
              setEditing(true);
            }}
          >
            {accountDisplayName(account)}
          </ActionButton>
        </>
      )}
    </div>
  );
}

export default Accounts;

/** Frame payloads use the same canonical readback-before-summary source as Accounts. */
export function frameAccounts(
  data: Pick<AccountsState, "accounts" | "defaultAccountKey">,
) {
  return data.accounts.map((account) => ({
    accountKey: account.id,
    provider: account.provider,
    email: account.email || null,
    plan: account.plan || null,
    status: account.status,
    label: account.label,
    isDefault: account.id === data.defaultAccountKey,
  }));
}

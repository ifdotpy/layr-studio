import { AccountStatus } from "../../../generated/api.ts";
import { makeAutoObservable } from "mobx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import type { ServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { ApiError, errorText, type PostBody } from "../../../api.ts";
import { ACCOUNT_NAME_MAX_CHARS } from "../../../generated/sharedValues.ts";
import { ProviderKind } from "../../../generated/providerKind.ts";
import type { ServerCommand } from "../../../servers/navigation.ts";
import type { Agent } from "../../../types.ts";
import type { AddAccountIntent } from "../AccountAddDialog/AccountAddDialog.tsx";
import { createAccountAddDraft } from "../AccountAddForm/AccountAddForm.tsx";
import {
  confirmAccountDeleteRequest,
  getOrCreateAccountDeleteRequest,
} from "../support/accountDeleteRequests.ts";

type Command = Extract<
  ServerCommand,
  { action: "account-sign-in" | "account-action" }
>;
type Dialog =
  | { kind: "add"; intent: AddAccountIntent }
  | {
      kind: "signIn" | "rename" | "configureProfile" | "disconnect" | "delete";
      accountKey: string;
      serverLabel?: string | undefined;
    }
  | {
      kind: "transfer";
      accountKey: string;
      agentId: string;
      sourceProvider: string;
      requestId: string;
    };

/** Transient account UI belongs to one server; account facts stay in generated models. */
export class AccountsUi {
  dialog: Dialog | null = null;
  pending = "";
  error = "";
  renameLabel = "";
  renameRequest: PostBody<"/api/accounts/name"> | null = null;
  addDraft = createAccountAddDraft();
  addFlow = false;
  private handled: Command | null = null;
  readonly areas: ServerAreas;
  constructor(areas: ServerAreas) {
    this.areas = areas;
    makeAutoObservable<this, "handled">(
      this,
      { areas: false, handled: false },
      { autoBind: true },
    );
  }
  get summary() {
    return this.areas.accounts;
  }
  get accounts() {
    return this.summary?.accounts || [];
  }
  get archivedAccounts() {
    return this.summary?.archivedAccounts || [];
  }
  get dialogAccount() {
    const key =
      this.dialog && "accountKey" in this.dialog ? this.dialog.accountKey : "";
    return this.accounts.find((a) => a.id === key);
  }
  get dialogOpen() {
    return this.dialog !== null;
  }
  get targetsDefault() {
    return this.dialogAccount?.id === this.summary?.defaultAccountKey;
  }
  get replacementAccount() {
    return this.accounts.find(
      (a) =>
        a.id !== this.dialogAccount?.id &&
        !a.disconnected &&
        a.status === AccountStatus.ready,
    );
  }
  get canConfirmRemoval() {
    return (
      !this.pending &&
      !!this.dialogAccount &&
      (!this.targetsDefault || !!this.replacementAccount)
    );
  }
  get trimmedLabel() {
    return this.renameLabel.trim();
  }
  get validationError() {
    return !this.trimmedLabel
      ? "Enter a name."
      : this.trimmedLabel.length > ACCOUNT_NAME_MAX_CHARS
        ? `Use at most ${ACCOUNT_NAME_MAX_CHARS} characters.`
        : "";
  }
  get canSave() {
    return !this.pending && (!!this.renameRequest || !this.validationError);
  }
  get addIntent() {
    return this.dialog?.kind === "add" ? this.dialog.intent : null;
  }
  get serverLabel() {
    return this.dialog && "serverLabel" in this.dialog
      ? this.dialog.serverLabel
      : this.addIntent?.serverLabel;
  }
  close() {
    if (!this.pending) this.dialog = null;
  }
  dispose() {
    this.dialog = null;
  }
  setRenameLabel(value: string) {
    if (!this.pending && !this.renameRequest) this.renameLabel = value;
  }
  continueAdd() {
    if (this.addDraft.provider) this.addFlow = true;
  }
  openAdd(intent: AddAccountIntent) {
    this.addDraft.provider = intent.provider || "";
    this.addDraft.label = intent.label || "";
    this.addDraft.email = intent.email || "";
    this.addFlow = !!intent.provider;
    this.dialog = { kind: "add", intent };
  }
  receive(command: Command | null | undefined) {
    if (!this.summary || !command || this.handled === command) return;
    if (this.pending) {
      this.error = "Wait for the current account action to finish.";
      return;
    }
    this.handled = command;
    this.error = "";
    if (command.action === "account-sign-in" && command.forceAdd) {
      this.openAdd(command);
      return;
    }
    const account = this.accounts.find(
      (a) => a.id === command.accountKey && a.provider === command.provider,
    );
    if (!account) {
      this.error = "This account is no longer saved on this server.";
      return;
    }
    if (command.action === "account-sign-in")
      this.dialog = {
        kind: "signIn",
        accountKey: account.id,
        serverLabel: command.serverLabel,
      };
    else if (command.operation === "default")
      void this.action(`default:${account.id}`, () =>
        this.areas.scope.services.api.post("/api/accounts/default", {
          account_key: account.id,
        }),
      );
    else if (command.operation === "rename") {
      this.renameRequest =
        this.areas.scope.services.api.saved<PostBody<"/api/accounts/name"> | null>(
          this.renameStorageKey(account.id),
          null,
        );
      this.renameLabel = this.renameRequest?.label || account.label;
      this.dialog = { kind: "rename", accountKey: account.id };
    } else if (command.operation === "configure-profile") {
      if (account.provider === ProviderKind.Claude)
        this.dialog = { kind: "configureProfile", accountKey: account.id };
      else
        this.error = "This Claude account is no longer saved on this server.";
    } else
      this.dialog = {
        kind: command.operation === "remove" ? "delete" : "disconnect",
        accountKey: account.id,
      };
  }
  *action(
    key: string,
    run: () => Promise<unknown>,
    success?: () => void,
    rejected?: () => void,
  ): Generator<Promise<unknown>, void, unknown> {
    if (this.pending) return;
    this.pending = key;
    this.error = "";
    try {
      yield run();
      success?.();
    } catch (failure) {
      if (
        failure instanceof ApiError &&
        failure.status >= 400 &&
        failure.status < 500 &&
        failure.status !== 408
      )
        rejected?.();
      this.error = errorText(failure);
    } finally {
      this.pending = "";
    }
  }
  renameStorageKey(key: string) {
    return `account-rename:${key}`;
  }
  saveRename() {
    const account = this.dialogAccount;
    if (!account || !this.canSave) return;
    const { api } = this.areas.scope.services;
    this.renameRequest ??= {
      account_key: account.id,
      label: this.trimmedLabel,
      request_id: randomUUID(),
    };
    const request = this.renameRequest;
    api.save(this.renameStorageKey(account.id), request);
    const clear = () => {
      api.save(this.renameStorageKey(account.id), null);
      this.renameRequest = null;
    };
    return this.action(
      `rename:${account.id}`,
      () => api.post("/api/accounts/name", request),
      () => {
        clear();
        this.dialog = null;
      },
      clear,
    );
  }

  confirmRemoval() {
    if (
      !this.canConfirmRemoval ||
      !this.dialog ||
      !["delete", "disconnect"].includes(this.dialog.kind)
    )
      return;
    const kind = this.dialog.kind,
      key = this.dialogAccount!.id;
    const { api, storage } = this.areas.scope.services;
    return this.action(
      kind,
      () => {
        if (kind === "delete")
          return confirmAccountDeleteRequest(
            this.areas.scope.services.owner.id,
            key,
            getOrCreateAccountDeleteRequest(
              this.areas.scope.services.owner.id,
              key,
              storage,
            ),
            (id) =>
              api.post("/api/accounts/delete", {
                account_key: key,
                request_id: id,
              }),
            storage,
          );
        return api.post("/api/accounts/disconnect", { account_key: key });
      },
      () => {
        this.dialog = null;
      },
    );
  }
  selectedAccount(key?: string) {
    return [...this.accounts, ...this.archivedAccounts].find(
      (a) => a.id === key,
    );
  }
  canSelectAccount(agent?: Agent) {
    return (
      !this.pending &&
      !!agent?.accountChange &&
      agent.accountChange.kind !== "unavailable"
    );
  }
  selectAccount(
    key: string,
    agent: Agent | undefined,
    accountKey: string | undefined,
    change?: (key: string) => Promise<void>,
  ) {
    if (
      !this.canSelectAccount(agent) ||
      key === accountKey ||
      !agent?.accountDestinations?.some(
        (d) => d.accountKey === key && d.eligibility.kind === "available",
      )
    )
      return;
    const account = this.accounts.find((a) => a.id === key);
    if (!account) return;
    if (agent.accountChange?.kind === "transfer" && agent.isLead) {
      const requestId = this.areas.scope.services.api.stableRequestId(
        `account-transfer:${agent.id}:${key}`,
      );
      this.dialog = {
        kind: "transfer",
        accountKey: key,
        agentId: agent.id,
        sourceProvider:
          this.selectedAccount(accountKey)?.provider || agent.provider || "",
        requestId,
      };
    } else if (agent.accountChange?.kind === "select")
      void this.action(key, () => change?.(key) || Promise.resolve());
  }
  confirmTransfer() {
    const choice = this.dialog;
    if (choice?.kind !== "transfer") return;
    return this.action(
      "transfer",
      () =>
        this.areas.scope.services.api.post("/api/agents/account-transfer", {
          id: choice.agentId,
          account_key: choice.accountKey,
          request_id: choice.requestId,
        }),
      () => {
        this.areas.scope.services.api.clearStableRequestId(
          `account-transfer:${choice.agentId}:${choice.accountKey}`,
        );
        this.dialog = null;
      },
    );
  }
}

import { makeAutoObservable } from "mobx";
import { AccountStatus } from "../generated/api.ts";
import { PROVIDER_LABELS } from "../generated/providerKind.ts";
import type { ServerAccount, ServerCommand } from "./navigation.ts";
import type { StudioServer } from "./registry.ts";
import type { AccountSource } from "./ServerAccountLimits.tsx";
import { serverAccounts } from "./ServerAccountLimits.tsx";
import {
  serverAccountRows,
  type ServerAccountRow,
} from "./serverAccountRows.ts";
import type { ServerServices } from "./serverServices.ts";
import { createAccountAddDraft } from "../components/accounts/AccountAddForm/AccountAddForm.tsx";
import type { Account } from "../components/accounts/support/Accounts.tsx";
import { MenuEscapeState } from "../components/ui/support/closeMenuFromTriggerOnEscape.ts";
import type { ServerAccountsPanelProps } from "../components/servers/ServerAccountsPanel/ServerAccountsPanel.tsx";
export type AccountRowInputs = Pick<
  ServerAccountsPanelProps,
  "servers" | "startAccount"
> & {
  accountsByServer: Record<string, ServerAccount[]>;
  statuses: Record<string, string>;
  servicesFor?: (id: string) => ServerServices | undefined;
  accountAction: (
    id: string,
    command: Extract<ServerCommand, { action: "account-action" }>,
  ) => void;
};
export function serverConnection(status?: string) {
  if (status === "offline") return { label: "Offline", blocked: true };
  if (status === "schema-mismatch")
    return { label: "Update required", blocked: true };
  if (status === "degraded")
    return { label: "Connection degraded", blocked: false };
  if (status === "connecting") return { label: "Connecting", blocked: false };
  if (status === "live") return null;
  return { label: "Connection unknown", blocked: false };
}

export function signInStatus(status: string) {
  if (status === "ready") return "Signed in";
  if (status === "signedOut" || status === "changed") return "Sign in needed";
  if (status === "error") return "Sign-in check failed";
  return "Sign-in status unknown";
}

/** Statuses where the inline sign-in button is shown; others use the account menu. */
export function needsSignIn(status: string) {
  return (
    status === AccountStatus.signedOut ||
    status === AccountStatus.changed ||
    status === AccountStatus.error
  );
}

/** The copies of one provider account on each server that has it, with the
 * server's full account record. A disconnected copy is left out. */
export function accountSources(
  row: ServerAccountRow,
  servers: StudioServer[],
  fullAccounts: Record<string, readonly Account[]>,
  servicesFor?: (serverId: string) => ServerServices | undefined,
): AccountSource[] {
  return servers.flatMap((server) => {
    const services = servicesFor?.(server.id);
    const source = accountSource(
      row,
      server,
      fullAccounts,
      services,
      !servicesFor || !!services,
    );
    return source ? [source] : [];
  });
}
function accountSource(
  row: ServerAccountRow,
  server: StudioServer,
  fullAccounts: Record<string, readonly Account[]>,
  services: ServerServices | undefined,
  available: boolean,
): AccountSource | undefined {
  const listed = row.byServer[server.id];
  const account =
    listed &&
    fullAccounts[server.id]?.find(
      (candidate) =>
        candidate.id === listed.accountKey &&
        candidate.provider === listed.provider,
    );
  return available && account && !account.disconnected
    ? { serverId: server.id, serverLabel: server.label, account, services }
    : undefined;
}

export class AccountsPanelState {
  escape = new MenuEscapeState();
  draft = createAccountAddDraft();
  adding = false;
  serverId: string | null = null;
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true });
  }
  setAdding = (value: boolean) => {
    this.adding = value;
  };
  setServerId = (value: string | null) => {
    this.serverId = value;
  };
  openAdd() {
    this.draft.provider = "";
    this.adding = true;
  }
  continueAdd(
    selected: StudioServer | undefined,
    start: (
      id: string,
      command: Extract<ServerCommand, { action: "account-sign-in" }>,
    ) => void,
  ) {
    const draft = this.draft;
    if (!selected || !draft.provider) return;
    start(selected.id, {
      action: "account-sign-in",
      provider: draft.provider,
      email: draft.email.trim() || null,
      label:
        draft.label.trim() ||
        draft.email.trim() ||
        PROVIDER_LABELS[draft.provider],
      serverLabel: selected.label,
      forceAdd: true,
    });
    this.adding = false;
    draft.label = "";
    draft.email = "";
  }
}

export function accountRowView(
  row: ServerAccountRow,
  input: AccountRowInputs,
  fullAccounts: Record<string, readonly Account[]>,
) {
  const markers: Record<AccountMarker["kind"], AccountMarker[]> = {
    connection: [],
    signIn: [],
    default: [],
  };
  const servers = input.servers.map((server) => {
    const account = row.byServer[server.id];
    const connection = serverConnection(input.statuses[server.id]);
    const services = input.servicesFor?.(server.id);
    const available = !input.servicesFor || !!services;
    const source = accountSource(
      row,
      server,
      fullAccounts,
      services,
      available,
    );
    const blocked =
      !!connection?.blocked || !available || (!!account && !account.accountKey);
    const state = blocked
      ? "offline"
      : account?.status === "ready"
        ? "ready"
        : account?.status === "error"
          ? "error"
          : account
            ? "reauth"
            : "missing";
    const description = account ? signInStatus(account.status) : "";
    const markerConnection = available ? connection : serverConnection();
    if (account) {
      if (markerConnection)
        markers.connection.push({
          kind: "connection",
          label: server.label + ": " + markerConnection.label,
        });
      if (account.status !== "ready")
        markers.signIn.push({
          kind: "signIn",
          label: server.label + ": " + description,
          prefix: server.label + ": ",
          detail: description,
        });
      if (account.isDefault)
        markers.default.push({
          kind: "default",
          label: "Default " + server.label,
        });
    }
    const chip =
      { ready: "✓ ", error: "⚠ ", reauth: "⚠ ", offline: "", missing: "+ " }[
        state
      ] + server.label;
    const suffix =
      state === "offline"
        ? connection?.label.toLocaleLowerCase() || "connection unknown"
        : state === "error" || state === "reauth"
          ? description.toLocaleLowerCase()
          : "";
    return {
      server,
      account,
      blocked,
      description,
      state,
      chip: chip + (suffix ? " · " + suffix : ""),
      source,
      needsSignIn:
        !!account && (!account.accountKey || needsSignIn(account.status)),
      signIn: () => {
        if (blocked) return;
        input.startAccount(server.id, {
          action: "account-sign-in",
          ...(account?.accountKey
            ? { accountKey: account.accountKey }
            : { forceAdd: true }),
          provider: row.provider,
          email: row.email,
          label: row.label,
          serverLabel: server.label,
        });
      },
      act: (
        operation: Extract<
          ServerCommand,
          { action: "account-action" }
        >["operation"],
      ) => {
        if (blocked || !account?.accountKey) return;
        input.accountAction(server.id, {
          action: "account-action",
          accountKey: account.accountKey,
          operation,
          provider: row.provider,
          email: row.email,
          label: row.label,
        });
      },
    };
  });
  const present = servers.filter((item) => item.account);
  return {
    row,
    sources: servers.flatMap((item) => (item.source ? [item.source] : [])),
    present,
    markers: Object.values(markers).flat(),
    chips: row.email && !row.serverScoped ? servers : present,
    facts: {
      Email: row.email || "Unavailable",
      Servers: present.map((item) => item.server.label).join(", "),
      Default:
        present
          .filter((item) => item.account?.isDefault)
          .map((item) => item.server.label)
          .join(", ") || "No",
      "Sign-in": present
        .map((item) => item.server.label + ": " + item.description)
        .join(", "),
    },
  };
}

export function accountRows(
  input: AccountRowInputs,
  store: ServerServices["store"],
) {
  const fullAccounts = Object.fromEntries(
    input.servers.map((server) => [
      server.id,
      serverAccounts(store, server.id, input.servicesFor),
    ]),
  );
  return serverAccountRows(input.servers, input.accountsByServer).map((row) =>
    accountRowView(row, input, fullAccounts),
  );
}
export type AccountRowView = ReturnType<typeof accountRowView>;

type AccountMarker = {
  kind: "connection" | "signIn" | "default";
  label: string;
  prefix?: string;
  detail?: string;
};

import { textField } from "../../../stores/fields.ts";
import { accountCanStart, accountDisplayName } from "../../../accountName.ts";
import { providerLabel } from "../../../generated/providerKind.ts";
import {
  newChatDefault,
  parseGroups,
} from "../../accounts/support/accountPreference.ts";
import { TextInput } from "@mantine/core";
import { AccountTiles } from "../../accounts/AccountTiles/AccountTiles.tsx";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import { SettingsRow } from "../../ui/SettingsRow/SettingsRow.tsx";
import { makeAutoObservable } from "mobx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import type { ProjectSave } from "../../../app/projects/ProjectSave.ts";
import type { ProjectModel } from "../../../app/contracts.ts";
import type { AccountsState } from "../../accounts/support/Accounts.tsx";
import { observer } from "mobx-react-lite";

type Props = {
  path: string;
  project?: ProjectModel | undefined;
  accounts: AccountsState;
  defaultAccountKey: string;
  saves: {
    account: ProjectSave<"/api/projects">;
    worker: ProjectSave<"/api/projects">;
    order: ProjectSave<"/api/projects">;
    defaultOrder: ProjectSave<"/api/accounts/default-groups">;
  };
};
const fields = {
  order: {
    id: "project-account-order",
    label: "Account order",
    aria: "Account order of this project",
    saveLabel: "Save account order",
    help: "New chats and workers prefer accounts of the first group, then the next group. Other accounts stay available.",
  },
  defaultOrder: {
    id: "default-account-order",
    label: "Default account order",
    aria: "Default account order of this server",
    saveLabel: "Save default account order",
    help: "Applies to every project on this server without its own account order.",
  },
  worker: {
    id: "project-worker-base",
    label: "Worker base",
    aria: "Default worker base ref",
    saveLabel: "Save worker base",
    help: undefined,
  },
};
type Field = keyof typeof fields;

function createProjectAccountDraft(initial: Props) {
  const revision = initial.project?.accountRevision || 0,
    revisions = {
      order: initial.project?.accountGroupsRevision || 0,
      worker: initial.project?.workerBaseRevision || 0,
    };
  const key = initial.project?.accountKey || initial.defaultAccountKey;
  return makeAutoObservable(
    {
      key,
      keys: [...(initial.project?.accountKeys || [key])],
      text: {
        order: (initial.project?.accountGroups || []).join(", "),
        defaultOrder: (initial.accounts.defaultAccountGroups || []).join(", "),
        worker: initial.project?.workerBaseRef || "",
      },
      view({ project, accounts, defaultAccountKey, saves }: Props) {
        const ready = accountCanStart(
            accounts.accounts.find((a) => a.id === this.key),
          ),
          accountsChanged =
            this.key !== (project?.accountKey || defaultAccountKey) ||
            JSON.stringify(this.keys) !==
              JSON.stringify(
                project?.accountKeys || [
                  project?.accountKey || defaultAccountKey,
                ],
              );
        const selectedAccounts = accounts.accounts.filter((a) =>
            this.keys.includes(a.id),
          ),
          memberships = [
            ...accounts.accounts.filter(
              (a) => !a.disconnected || this.keys.includes(a.id),
            ),
            ...(accounts.archivedAccounts || []).filter((a) =>
              this.keys.includes(a.id),
            ),
          ];
        const baseline = {
          order: (project?.accountGroups || []).join(", "),
          defaultOrder: (accounts.defaultAccountGroups || []).join(", "),
          worker: project?.workerBaseRef || "",
        };
        const groups = [
          ...new Set(accounts.accounts.map((a) => a.group).filter(Boolean)),
        ].sort();
        const field = (kind: Field) => {
          const value = this.text[kind],
            parsed = parseGroups(value),
            save = saves[kind],
            error = kind === "worker" ? "" : parsed.error;
          return {
            ...fields[kind],
            kind,
            save,
            error,
            parsed,
            value,
            binding: textField(this.text, kind),
            changed:
              kind === "worker"
                ? value.trim() !== baseline.worker
                : parsed.groups.join(",") !==
                  parseGroups(baseline[kind]).groups.join(","),
            disabled: save.frozen ? !save.retryLabel : !!error,
            primary:
              kind === "order" ||
              (kind === "worker" && !accountsChanged && !saves.account.frozen),
            placeholder:
              kind === "worker"
                ? "main"
                : kind === "order" && baseline.defaultOrder
                  ? `Default order: ${baseline.defaultOrder}`
                  : "personal, work",
            description:
              kind !== "order"
                ? undefined
                : groups.length
                  ? `Groups on this server: ${groups.join(", ")}. Leave empty to use the default order.`
                  : "No account has a group yet. Set a group on each account in Settings > Accounts.",
          };
        };
        const preview = newChatDefault(
            accounts.accounts,
            accounts.defaultAccountKey,
            parseGroups(this.text.defaultOrder).groups,
            {
              accountKey: project?.accountKey,
              accountKeys: project?.accountKeys,
              accountGroups: parseGroups(this.text.order).groups,
            },
            null,
          ),
          account = accounts.accounts.find((a) => a.id === preview.accountKey);
        return {
          ready,
          accountsChanged,
          selectedAccounts,
          memberships,
          providers: [...new Set(memberships.map((a) => a.provider))],
          fields: {
            order: field("order"),
            defaultOrder: field("defaultOrder"),
            worker: field("worker"),
          },
          preview: account
            ? `New chats start on ${accountDisplayName(account)} (${providerLabel(account.provider)})${preview.model ? " with Opus 5.5 · Medium" : ""}.`
            : "No account can start a new chat. Sign in to an account first.",
        };
      },
      setKey(key: string) {
        this.key = key;
      },
      setKeys(keys: string[]) {
        this.keys = keys;
        if (!keys.includes(this.key)) this.key = "";
      },
      accountDisabled(account: AccountsState["accounts"][number]) {
        return !accountCanStart(account) && !this.keys.includes(account.id);
      },
      submit(props: Props): void {
        const { ready, accountsChanged } = this.view(props);
        if (!props.saves.account.frozen && (!ready || !accountsChanged)) return;
        void props.saves.account.submit({
          action: "set_accounts",
          path: props.path,
          account_keys: this.keys,
          account_key: this.key,
          expected_revision: revision,
        });
      },
      submitText(kind: Field, props: Props): void {
        const field = this.view(props).fields[kind];
        if (
          field.save.pending ||
          field.disabled ||
          (!field.save.frozen && !field.changed)
        )
          return;
        const { path, saves } = props;
        if (kind === "defaultOrder")
          void saves.defaultOrder.submit({ groups: field.parsed.groups });
        else if (kind === "order")
          void saves.order.submit({
            action: "set_account_groups",
            path,
            account_groups: field.parsed.groups,
            expected_revision: revisions.order,
          });
        else
          void saves.worker.submit({
            action: "set_worker_base",
            path,
            base_ref: field.value.trim() || null,
            expected_revision: revisions.worker,
          });
      },
    },
    { view: false },
    { autoBind: true },
  );
}
type Draft = ReturnType<typeof createProjectAccountDraft>;
const TextSetting = observer(function TextSetting({
  field,
  onSubmit,
}: {
  field: ReturnType<Draft["view"]>["fields"][Field];
  onSubmit: () => void;
}) {
  const { save, kind } = field;
  return (
    <form
      className="project-settings-group"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <SettingsRow
        label={<label htmlFor={field.id}>{field.label}</label>}
        help={field.help}
      >
        <TextInput
          id={field.id}
          aria-label={field.aria}
          placeholder={field.placeholder}
          description={field.description}
          aria-description={
            kind === "worker"
              ? "Use a branch, tag, or commit for new worker worktrees. Leave empty to use repository HEAD."
              : undefined
          }
          maxLength={kind === "worker" ? 1024 : undefined}
          {...field.binding}
          error={field.error || undefined}
          disabled={save.pending || save.frozen}
        />
      </SettingsRow>
      {save.error && (
        <p role="alert" className="account-action-error">
          {save.error}
        </p>
      )}
      {(save.frozen || field.changed) && (
        <ActionButton
          type="submit"
          aria-label={field.saveLabel}
          actionRole={field.primary ? "primary" : "secondary"}
          loading={save.pending}
          disabled={field.disabled}
        >
          {save.retryLabel || "Save"}
        </ActionButton>
      )}
    </form>
  );
});
export default observer(function ProjectAccount(props: Props) {
  const save = props.saves.account;
  return (
    <Owned
      key={JSON.stringify([props.path, props.project?.id])}
      create={() => createProjectAccountDraft(props)}
    >
      {(draft) => {
        const view = draft.view(props);
        return (
          <div className="project-settings">
            <p className="project-settings-path" title={props.path}>
              {props.path}
            </p>
            <form
              className="project-settings-group"
              onSubmit={(event) => {
                event.preventDefault();
                draft.submit(props);
              }}
            >
              <SettingsRow
                label="Default account"
                help="Comes first in its group of the account order. A Claude account starts new chats when one is ready."
              >
                <AccountTiles
                  label="Default account for new chats"
                  showLabel={false}
                  accounts={view.selectedAccounts}
                  value={view.ready ? draft.key : ""}
                  disabled={save.pending || save.frozen}
                  accountDisabled={(account) => !accountCanStart(account)}
                  onChange={draft.setKey}
                />
              </SettingsRow>
              <SettingsRow label="Shown first">
                <AccountTiles
                  label="Accounts shown first for this project"
                  showLabel={false}
                  multiple
                  providers={view.providers}
                  accounts={view.memberships}
                  value={draft.keys}
                  disabled={save.pending || save.frozen}
                  accountDisabled={draft.accountDisabled}
                  onChange={draft.setKeys}
                />
              </SettingsRow>
              {!view.ready && !save.frozen && (
                <p role="status">
                  Choose an account that is ready before you save.
                </p>
              )}
              {save.error && (
                <p role="alert" className="account-action-error">
                  {save.error}
                </p>
              )}
              {(save.frozen || view.accountsChanged) && (
                <ActionButton
                  aria-label="Save accounts"
                  actionRole="primary"
                  type="submit"
                  loading={save.pending}
                  disabled={
                    !save.frozen &&
                    (!draft.keys.length || !draft.key || !view.ready)
                  }
                >
                  {save.retryLabel || "Save"}
                </ActionButton>
              )}
            </form>
            <TextSetting
              field={view.fields.order}
              onSubmit={() => draft.submitText("order", props)}
            />
            <TextSetting
              field={view.fields.defaultOrder}
              onSubmit={() => draft.submitText("defaultOrder", props)}
            />
            <p className="project-settings-path" role="status">
              {view.preview}
            </p>
            <TextSetting
              field={view.fields.worker}
              onSubmit={() => draft.submitText("worker", props)}
            />
          </div>
        );
      }}
    </Owned>
  );
});

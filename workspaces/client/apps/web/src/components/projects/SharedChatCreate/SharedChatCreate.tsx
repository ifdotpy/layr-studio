import { useState } from "react";
import { whileObserved } from "../../../observed.ts";
import { resourceData } from "../../../store/runtime.ts";
import { textField } from "../../../stores/fields.ts";
import type {
  CreationOwner,
  SharedCreation as Creation,
} from "../../../app/projects/CreationOwner.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  isModelEnabled,
  readModelChoices,
} from "../../settings/models/support/modelChoices.ts";
import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { accountCanStart } from "../../../accountName.ts";
import { ProviderMark } from "../../accounts/ProviderMark/ProviderMark.tsx";
import { setupAccountName } from "../../accounts/AccountTiles/AccountTiles.tsx";
import { Button, NativeSelect, Popover, TextInput } from "@mantine/core";
import { makeAutoObservable } from "mobx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { useServerStorage } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  ApiError,
  errorText,
  type PostBody,
  type PostResult,
} from "../../../api.ts";
import type { StateCollections } from "../../../generated/observableResources.ts";
export type SharedCreationSource = {
  readonly creation: Pick<CreationOwner, "sharedAttempt" | "rememberShared">;
  readonly startup: { readonly creationScope: string };
  readonly state:
    | Pick<StateCollections, "projects" | "agents" | "rooms">
    | undefined;
};
import type { AccountsState } from "../../accounts/support/Accounts.tsx";
import {
  workerCatalogKey,
  loadingCatalog,
  emptyCatalog,
  type useWorkerModels,
} from "../../agents/support/WorkerModelPicker.tsx";
import {
  AgentSetupPicker,
  type SetupRole,
  type AgentSetupPickerProps,
} from "../../agents/AgentSetupPicker/AgentSetupPicker.tsx";
import {
  INHERITED_MODEL,
  modelDisplayName,
  modelForValue,
  toModelOptions,
} from "../../settings/models/support/modelOption.ts";

import { observer } from "mobx-react-lite";

type Participant = {
  account_key: string;
  model: string;
  effort?: string | null | undefined;
};
type SharedCreateRequest = Extract<
  PostBody<"/api/peer-teams">,
  { action: "radio"; radio_action: "create" }
>;

const ParticipantFields = observer(function ParticipantFields({
  form,
  index,
}: {
  form: SharedChatDraft;
  index: number;
}) {
  const [role, setRole] = useState<SetupRole>("orchestrator"),
    [provider, setProvider] = useState(form.participantProvider(index));
  const view = form.participantView(
    index,
    role,
    provider,
    setRole,
    setProvider,
  );
  if (!view) return null;
  return (
    <div
      className="shared-create-row shared-create-participant"
      onFocusCapture={(event) => {
        if (view.opened && event.target instanceof HTMLElement)
          event.target.setAttribute("data-mantine-stop-propagation", "true");
      }}
    >
      <span>Agent {index + 1}</span>
      <Popover
        opened={view.opened && !view.frozen}
        onChange={(opened) => form.setOpened(index, opened)}
        position={index === 0 ? "top-end" : "bottom-end"}
        closeOnEscape
        trapFocus
        returnFocus
      >
        <Popover.Target>
          <Button
            variant="subtle"
            type="button"
            className="shared-create-chip"
            aria-label={`Settings for agent ${index + 1}`}
            aria-expanded={view.opened && !view.frozen}
            disabled={view.frozen}
            leftSection={<ProviderMark provider={view.provider} />}
            onClick={() => form.setOpened(index, !view.opened)}
          >
            <span>{view.summary}</span>
          </Button>
        </Popover.Target>
        <Popover.Dropdown className="execution-dropdown shared-create-picker">
          <AgentSetupPicker {...view.picker} />
          {view.catalog.error && (
            <div role="alert">
              {view.catalog.error}{" "}
              <Button
                type="button"
                size="compact-xs"
                onClick={view.catalog.retry}
              >
                Retry models
              </Button>
            </div>
          )}
        </Popover.Dropdown>
      </Popover>
    </div>
  );
});
function createSharedChatDraft(
  source: SharedCreationSource,
  accounts: AccountsState,
  initialPath: string | undefined,
  catalogs: ReturnType<typeof useServerAreas>["agentsUi"],
  storage: ReturnType<typeof useServerStorage>,
  post: ReturnType<typeof useServerApi>["post"],
) {
  const scope = source.startup.creationScope;
  let active = false;
  const attempt = source.creation.sharedAttempt,
    drafts: Participant[] = [];
  const initial: { openParticipant: number | null } = { openParticipant: null };
  const form = makeAutoObservable(
    {
      drafts,
      ...initial,
      path:
        attempt?.body.path ||
        initialPath ||
        [...(source.state?.projects.values() ?? [])][0]?.path ||
        "",
      name: attempt?.body.name || "",
      pending: false,
      error: "",
      get accounts(): AccountsState {
        return (
          (catalogs.scope &&
            resourceData(
              catalogs.scope.store.state().current,
            )?.accountsSummary.get("current")) ??
          accounts
        );
      },
      get attempt() {
        return source.creation.sharedAttempt;
      },
      get readyAccounts(): AccountsState["accounts"] {
        return this.accounts.accounts.filter(accountCanStart);
      },
      get first(): AccountsState["accounts"][number] | undefined {
        return (
          this.readyAccounts.find(
            (a) => a.id === this.accounts.defaultAccountKey,
          ) || this.readyAccounts[0]
        );
      },
      get second(): AccountsState["accounts"][number] | undefined {
        return (
          this.readyAccounts.find((a) => a.provider !== this.first?.provider) ||
          this.first
        );
      },
      get selections(): {
        participant: Participant;
        ready: boolean;
        catalog: ReturnType<typeof useWorkerModels>;
      }[] {
        return this.drafts.map((draft, index) => {
          const account_key =
              draft.account_key ||
              (this.attempt
                ? ""
                : (index ? this.second : this.first)?.id || ""),
            provider =
              this.readyAccounts.find((account) => account.id === account_key)
                ?.provider || "",
            catalog = account_key
              ? (catalogs.workerCatalogs.get(workerCatalogKey(account_key)) ??
                loadingCatalog)
              : emptyCatalog;
          const models = catalog.models.filter((row) =>
            isModelEnabled(
              { ...row, provider: row.provider || provider },
              readModelChoices(storage),
            ),
          );
          const model =
            draft.model ||
            (this.attempt
              ? ""
              : (models.find((row) => row.isDefault) || models[0])?.model ||
                "");
          return {
            participant: { ...draft, account_key, model },
            ready:
              !catalog.loading &&
              !catalog.error &&
              models.some((row) => row.model === model),
            catalog,
          };
        });
      },
      participantProvider(index: number) {
        return (
          this.accounts.accounts.find(
            (account) => account.id === this.participants[index]?.account_key,
          )?.provider || ""
        );
      },
      participantView(
        index: number,
        role: SetupRole,
        browsedProvider: string,
        onRole: (value: SetupRole) => void,
        onProvider: (value: string) => void,
      ) {
        const row = this.selections[index];
        if (!row) return null;
        const { participant: value, catalog } = row,
          account = this.accounts.accounts.find(
            (account) => account.id === value.account_key,
          ),
          provider = account?.provider || "",
          info = modelForValue(catalog.models, value.model),
          reasoning = info?.supportedReasoningEfforts ?? [],
          effort = value.effort || info?.defaultReasoningEffort || "Default",
          frozen = !!this.attempt,
          readOnlyReason =
            role === "orchestrator"
              ? undefined
              : "Shared chat participants manage their own worker and review defaults. These settings are inherited and cannot be changed here.";
        const picker: AgentSetupPickerProps = {
          retainCurrentModel: false,
          role,
          onRole,
          readOnlyReason,
          provider: browsedProvider,
          providers: [
            ...new Set(this.readyAccounts.map((account) => account.provider)),
          ],
          onProvider,
          accounts: [...this.readyAccounts],
          accountKey: value.account_key,
          accountLabel: `Account for agent ${index + 1}`,
          onAccount: (account_key) => {
            if (account_key && account_key !== value.account_key)
              this.changeParticipant(index, { account_key, model: "" });
          },
          models: readOnlyReason
            ? [
                {
                  value: INHERITED_MODEL,
                  label: "Inherited",
                  provider,
                  badges: [],
                },
              ]
            : toModelOptions(catalog.models, { provider }),
          model: readOnlyReason ? INHERITED_MODEL : value.model,
          modelLabel: `Model for agent ${index + 1}`,
          onModel: (model) =>
            this.changeParticipant(index, {
              ...value,
              model,
              effort: undefined,
            }),
          efforts:
            !readOnlyReason && reasoning.length
              ? [
                  { value: "", label: "Default" },
                  ...reasoning.map(({ reasoningEffort: item }) => ({
                    value: item,
                    label: item.charAt(0).toUpperCase() + item.slice(1),
                  })),
                ]
              : [],
          effort: value.effort || "",
          effortLabel: `Reasoning for agent ${index + 1}`,
          onEffort: (effort) =>
            this.changeParticipant(index, {
              ...value,
              effort: effort || undefined,
            }),
          disabled: frozen || catalog.loading || !!catalog.error,
          accountDisabled: frozen,
          fastAvailable: false,
          fast: false,
          onFast: () => {},
          daybreakAvailable: false,
          daybreak: false,
          onDaybreak: () => {},
        };
        return {
          picker,
          catalog,
          provider,
          frozen,
          opened: this.openParticipant === index,
          summary: [
            modelDisplayName(info, value.model) ||
              (catalog.loading ? "Loading models…" : "Choose a model"),
            effort.charAt(0).toUpperCase() + effort.slice(1),
            setupAccountName(account),
          ]
            .filter(Boolean)
            .join(" · "),
        };
      },
      changeParticipant(index: number, value: Participant) {
        this.drafts = this.drafts.map((draft, i) =>
          i === index ? value : draft,
        );
      },
      setOpened(index: number, opened: boolean) {
        this.openParticipant = opened
          ? index
          : this.openParticipant === index
            ? null
            : this.openParticipant;
      },
      get participants(): Participant[] {
        return this.selections.map((row) => row.participant);
      },
      get disabled(): boolean {
        return (
          this.pending ||
          (!this.attempt &&
            (!this.path ||
              !this.selections.every((row) => row.ready) ||
              this.participants.some(
                (p) =>
                  !p.model ||
                  !this.readyAccounts.some((a) => a.id === p.account_key),
              )))
        );
      },
      *submit(): Generator<
        ReturnType<ReturnType<typeof useServerApi>["post"]>,
        void,
        PostResult<"/api/peer-teams">
      > {
        if (this.pending) return;
        this.pending = true;

        this.error = "";
        const { path, name, participants, attempt } = this;
        const remember = (value: Creation | null) =>
          source.creation.rememberShared(value, scope);
        const body: SharedCreateRequest = {
          action: "radio",
          radio_action: "create",
          request_id: randomUUID(),
          path,
          name: name.trim() || "Shared chat",
          participants,
        };
        let next: Creation = attempt || { body };
        remember(next);
        try {
          if (!next.roomId && !next.rejected) {
            const result = yield post("/api/peer-teams", next.body, {
              timeoutMs: 15000,
            });
            const room = "room" in result ? result.room : null;
            if (
              !room ||
              typeof room !== "object" ||
              Array.isArray(room) ||
              !("id" in room) ||
              typeof room.id !== "string"
            )
              throw Error(
                "The server did not return the shared chat identity.",
              );
            next = { ...next, roomId: room.id };
            remember(next);
          }
          if (next.rejected) remember(null);
        } catch (e) {
          if (
            !next.roomId &&
            e instanceof ApiError &&
            e.status >= 400 &&
            e.status < 500
          ) {
            next = { ...next, rejected: true };
            remember(next);
          }
          if (active) this.error = errorText(e);
        } finally {
          this.pending = false;
        }
      },

      get projects() {
        const projects = new Map<string, { value: string; label: string }>();
        for (const { path, name } of source.state?.projects.values() || []) {
          if (path)
            projects.set(path, {
              value: path,
              label: name || path.split("/").filter(Boolean).pop() || path,
            });
        }
        for (const { projectPath: path } of source.state?.agents.values() ||
          []) {
          if (path) projects.set(path, { value: path, label: path });
        }
        return projects;
      },
    },
    {
      participantView: false,
      participantProvider: false,
    },
    { autoBind: true },
  );
  form.drafts = attempt?.body.participants || [
    { account_key: form.first?.id || "", model: "" },
    { account_key: form.second?.id || "", model: "" },
  ];
  whileObserved(form, "pending", () => {
    active = true;
    return () => {
      active = false;
    };
  });
  return form;
}
type SharedChatDraft = ReturnType<typeof createSharedChatDraft>;
export default observer(function SharedChatCreate({
  source,
  accounts,
  initialPath,
}: {
  source: SharedCreationSource;
  accounts: AccountsState;
  initialPath?: string | undefined;
}) {
  const { post } = useServerApi(),
    catalogs = useServerAreas().agentsUi,
    storage = useServerStorage();
  return (
    <Owned
      key={`${catalogs.scope?.services.owner.id || ""}:${source.startup.creationScope}`}
      create={() =>
        createSharedChatDraft(
          source,
          accounts,
          initialPath,
          catalogs,
          storage,
          post,
        )
      }
    >
      {(form) => (
        <form
          className="shared-chat-create"
          aria-label="Create shared chat"
          onSubmit={(e) => {
            e.preventDefault();
            void form.submit();
          }}
        >
          {(form.projects.size > 1 || !form.path) && (
            <div className="shared-create-row">
              <span>Project</span>
              <NativeSelect
                aria-label="Project"
                {...textField(form, "path")}
                disabled={!!form.attempt}
                data={[
                  { value: "", label: "Choose a project" },
                  ...form.projects.values(),
                ]}
              />
            </div>
          )}
          <div className="shared-create-row">
            <span>Name</span>
            <TextInput
              aria-label="Chat name"
              placeholder="Optional"
              {...textField(form, "name")}
              disabled={!!form.attempt}
              maxLength={80}
            />
          </div>
          <div className="shared-create-participants">
            {form.participants.map((value, index) => (
              <ParticipantFields
                key={`${index}:${value.account_key}`}
                form={form}
                index={index}
              />
            ))}
          </div>
          {!form.readyAccounts.length && (
            <p role="alert">
              Connect an account before you create a shared chat.
            </p>
          )}
          {form.attempt && (
            <p role={form.error ? "alert" : "status"}>
              {form.error}{" "}
              {form.attempt.roomId
                ? "Created. Refresh to open the chat."
                : form.attempt.rejected
                  ? "Request rejected. Refresh before you edit."
                  : "Waiting for confirmation. A retry uses the same request."}
            </p>
          )}
          <div className="shared-create-footer">
            <Button
              variant="filled"
              aria-label={form.attempt ? undefined : "Create"}
              type="submit"
              loading={form.pending}
              disabled={form.disabled}
            >
              {form.attempt
                ? form.attempt.roomId || form.attempt.rejected
                  ? "Refresh"
                  : "Retry creation"
                : "Create"}
            </Button>
          </div>
        </form>
      )}
    </Owned>
  );
});

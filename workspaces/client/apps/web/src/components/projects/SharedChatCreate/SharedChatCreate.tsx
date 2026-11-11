import type {
  CreationOwner,
  SharedCreation as Creation,
} from "../../../app/projects/CreationOwner.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  isModelEnabled,
  useModelChoices,
} from "../../settings/models/support/modelChoices.ts";
import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { accountCanStart } from "../../../accountName.ts";
import { ProviderMark } from "../../accounts/ProviderMark/ProviderMark.tsx";
import { setupAccountName } from "../../accounts/AccountTiles/AccountTiles.tsx";
import { Button, NativeSelect, Popover, TextInput } from "@mantine/core";
import { useRef, useState } from "react";
import { ApiError, errorText, type PostBody } from "../../../api.ts";
import type { StateCollections } from "../../../generated/observableResources.ts";
export type SharedCreationSource = {
  readonly creation: Pick<CreationOwner, "sharedAttempt" | "rememberShared">;
  readonly startup: { readonly creationScope: string };
  readonly state:
    | Pick<StateCollections, "projects" | "agents" | "rooms">
    | undefined;
};
import type { AccountsState } from "../../accounts/support/Accounts.tsx";
import { useWorkerModels } from "../../agents/support/WorkerModelPicker.tsx";
import {
  AgentSetupPicker,
  type SetupRole,
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
  index,
  value,
  change,
  accounts,
  frozen,
  opened,
  onOpenedChange,
  catalog,
}: {
  index: number;
  value: Participant;
  change: (value: Participant) => void;
  accounts: AccountsState;
  frozen: boolean;
  opened: boolean;
  onOpenedChange: (opened: boolean) => void;
  catalog: ReturnType<typeof useWorkerModels>;
}) {
  const [role, setRole] = useState<SetupRole>("orchestrator");
  const readOnlyReason =
    role === "orchestrator"
      ? undefined
      : "Shared chat participants manage their own worker and review defaults. These settings are inherited and cannot be changed here.";
  const readyAccounts = accounts.accounts.filter((account) =>
    accountCanStart(account),
  );
  const account = accounts.accounts.find(
    (item) => item.id === value.account_key,
  );
  const provider = account?.provider || "";
  const [browsedProvider, setBrowsedProvider] = useState(provider);
  const providers = [...new Set(readyAccounts.map((item) => item.provider))];
  const chooseAccount = (account_key: string | null) => {
    if (account_key && account_key !== value.account_key)
      change({ account_key, model: "" });
  };
  const info = modelForValue(catalog.models, value.model);
  const reasoningEfforts =
    info?.supportedReasoningEfforts.map((item) => item.reasoningEffort) ?? [];
  const effort = value.effort || info?.defaultReasoningEffort || "Default";
  const summary = [
    modelDisplayName(info, value.model) ||
      (catalog.loading ? "Loading models…" : "Select a model"),
    effort.charAt(0).toUpperCase() + effort.slice(1),
    setupAccountName(account),
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <div
      className="shared-create-row shared-create-participant"
      onFocusCapture={(event) => {
        if (opened && event.target instanceof HTMLElement)
          event.target.setAttribute("data-mantine-stop-propagation", "true");
      }}
    >
      <span>Agent {index + 1}</span>
      <Popover
        opened={opened && !frozen}
        onChange={onOpenedChange}
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
            aria-expanded={opened && !frozen}
            disabled={frozen}
            leftSection={<ProviderMark provider={provider} />}
            onClick={() => onOpenedChange(!opened)}
          >
            <span>{summary}</span>
          </Button>
        </Popover.Target>
        <Popover.Dropdown className="execution-dropdown shared-create-picker">
          <AgentSetupPicker
            retainCurrentModel={false}
            role={role}
            onRole={setRole}
            readOnlyReason={readOnlyReason}
            provider={browsedProvider}
            providers={providers}
            onProvider={setBrowsedProvider}
            accounts={readyAccounts}
            accountKey={value.account_key}
            accountLabel={`Account for agent ${index + 1}`}
            onAccount={chooseAccount}
            models={
              readOnlyReason
                ? [
                    {
                      value: INHERITED_MODEL,
                      label: "Inherited",
                      provider,
                      badges: [],
                    },
                  ]
                : toModelOptions(catalog.models, { provider })
            }
            model={readOnlyReason ? INHERITED_MODEL : value.model}
            modelLabel={`Model for agent ${index + 1}`}
            onModel={(model) => change({ ...value, model, effort: undefined })}
            efforts={
              !readOnlyReason && reasoningEfforts.length
                ? [
                    { value: "", label: "Default" },
                    ...reasoningEfforts.map((item) => ({
                      value: item,
                      label: item.charAt(0).toUpperCase() + item.slice(1),
                    })),
                  ]
                : []
            }
            effort={value.effort || ""}
            effortLabel={`Reasoning for agent ${index + 1}`}
            onEffort={(next) => change({ ...value, effort: next || undefined })}
            disabled={frozen || catalog.loading || !!catalog.error}
            accountDisabled={frozen}
            fastAvailable={false}
            fast={false}
            onFast={() => {}}
            daybreakAvailable={false}
            daybreak={false}
            onDaybreak={() => {}}
          />
          {catalog.error && (
            <div role="alert">
              {catalog.error}{" "}
              <Button type="button" size="compact-xs" onClick={catalog.retry}>
                Retry models
              </Button>
            </div>
          )}
        </Popover.Dropdown>
      </Popover>
    </div>
  );
});
export default observer(function SharedChatCreate({
  source,
  accounts,
  initialPath,
}: {
  source: SharedCreationSource;
  accounts: AccountsState;
  initialPath?: string | undefined;
}) {
  const { post } = useServerApi();

  const attempt = source.creation.sharedAttempt;
  const remember = source.creation.rememberShared;
  const [path, setPath] = useState<string>(
    attempt?.body.path ||
      initialPath ||
      [...(source.state?.projects.values() ?? [])][0]?.path ||
      "",
  );
  const [name, setName] = useState<string>(attempt?.body.name || "");
  const readyAccounts = accounts.accounts.filter((a) => accountCanStart(a));
  const first =
    readyAccounts.find((a) => a.id === accounts.defaultAccountKey) ||
    readyAccounts[0];
  const second =
    readyAccounts.find((a) => a.provider !== first?.provider) || first;
  const [drafts, setParticipants] = useState<Participant[]>(
    attempt?.body.participants || [
      { account_key: first?.id || "", model: "" },
      { account_key: second?.id || "", model: "" },
    ],
  );
  const accountsForParticipants = drafts.map(
    (p, index) =>
      p.account_key || (attempt ? "" : (index ? second : first)?.id || ""),
  );
  const catalogs = [
    useWorkerModels(
      accountsForParticipants[0] || "",
      !!accountsForParticipants[0],
    ),
    useWorkerModels(
      accountsForParticipants[1] || "",
      !!accountsForParticipants[1],
    ),
  ];
  const allowed = useModelChoices();
  const selections = drafts.map((p, index) => {
    const account_key = accountsForParticipants[index] || "";
    const provider =
      readyAccounts.find((a) => a.id === account_key)?.provider || "";
    const catalog = catalogs[index]!;
    const models = catalog.models.filter((row) =>
      isModelEnabled({ ...row, provider: row.provider || provider }, allowed),
    );
    const model =
      p.model ||
      (attempt
        ? ""
        : (models.find((row) => row.isDefault) || models[0])?.model || "");
    return {
      participant: { ...p, account_key, model },
      ready:
        !catalog.loading &&
        !catalog.error &&
        models.some((row) => row.model === model),
    };
  });
  const participants = selections.map((row) => row.participant);
  const valid = selections.every((row) => row.ready);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [openParticipant, setOpenParticipant] = useState<number | null>(null);
  const lock = useRef(false);
  const submit = async () => {
    if (lock.current) return;
    lock.current = true;
    setPending(true);
    setError("");
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
        const result = await post("/api/peer-teams", next.body, {
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
          throw Error("The server did not return the shared chat identity.");
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
      setError(errorText(e));
    } finally {
      lock.current = false;
      setPending(false);
    }
  };
  const projects = new Map<string, { value: string; label: string }>();
  for (const { path, name } of source.state?.projects.values() || []) {
    if (path)
      projects.set(path, {
        value: path,
        label: name || path.split("/").filter(Boolean).pop() || path,
      });
  }
  for (const { projectPath: path } of source.state?.agents.values() || []) {
    if (path) projects.set(path, { value: path, label: path });
  }
  return (
    <form
      className="shared-chat-create"
      aria-label="Create shared chat"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      {(projects.size > 1 || !path) && (
        <div className="shared-create-row">
          <span>Project</span>
          <NativeSelect
            aria-label="Project"
            value={path}
            disabled={!!attempt}
            data={[
              { value: "", label: "Select a project" },
              ...projects.values(),
            ]}
            onChange={(e) => setPath(e.currentTarget.value)}
          />
        </div>
      )}
      <div className="shared-create-row">
        <span>Name</span>
        <TextInput
          aria-label="Chat name"
          placeholder="Optional"
          value={name}
          disabled={!!attempt}
          maxLength={80}
          onChange={(e) => setName(e.currentTarget.value)}
        />
      </div>
      <div className="shared-create-participants">
        {participants.map((value, index) => (
          <ParticipantFields
            key={`${index}:${value.account_key}`}
            index={index}
            value={value}
            accounts={accounts}
            frozen={!!attempt}
            opened={openParticipant === index}
            onOpenedChange={(opened) =>
              setOpenParticipant((current) =>
                opened ? index : current === index ? null : current,
              )
            }
            change={(next) =>
              setParticipants((old) =>
                old.map((p, i) => (i === index ? next : p)),
              )
            }
            catalog={catalogs[index]!}
          />
        ))}
      </div>
      {!readyAccounts.length && (
        <p role="alert">Connect an account before you create a shared chat.</p>
      )}
      {attempt && (
        <p role={error ? "alert" : "status"}>
          {error}{" "}
          {attempt.roomId
            ? "Created. Refresh to open the chat."
            : attempt.rejected
              ? "Request rejected. Refresh before you edit."
              : "Waiting for confirmation. A retry uses the same request."}
        </p>
      )}
      <div className="shared-create-footer">
        <Button
          variant="filled"
          aria-label={attempt ? undefined : "Create"}
          type="submit"
          loading={pending}
          disabled={
            pending ||
            (!attempt &&
              (!path ||
                !valid ||
                participants.some((p) => !p.model) ||
                participants.some(
                  (p) => !readyAccounts.some((a) => a.id === p.account_key),
                )))
          }
        >
          {attempt
            ? attempt.roomId || attempt.rejected
              ? "Refresh"
              : "Retry creation"
            : "Create"}
        </Button>
      </div>
    </form>
  );
});

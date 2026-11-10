import { observer } from "mobx-react-lite";
import {
  AgentSource as GeneratedApiAgentSource,
  AgentStatus as GeneratedApiAgentStatus,
} from "../../../generated/api.ts";
import {
  isModelEnabled,
  useModelChoices,
} from "../models/support/modelChoices.ts";
import { modelForValue } from "../models/support/modelOption.ts";
import {
  ServerServicesProvider,
  useServerServices,
  useServerStore,
  useServerSync,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  createServerServices,
  type ServerServices,
} from "../../../servers/serverServices.ts";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Stack } from "@mantine/core";
import { errorText } from "../../../api.ts";
import { resourceData } from "../../../store/runtime.ts";
import type { Agent } from "../../../types.ts";
import type { StudioServer } from "../../../servers/registry.ts";
import {
  ExecutionSettings,
  settingsFor,
} from "../../agents/ExecutionSettings/ExecutionSettings.tsx";
import { useWorkerModels } from "../../agents/support/WorkerModelPicker.tsx";
import {
  DEFAULT_CLAUDE_LEAD_EFFORT,
  DEFAULT_CLAUDE_LEAD_MODEL,
} from "../../../generated/sharedValues.ts";
import {
  defaultChatModel,
  newChatDefault,
} from "../../accounts/support/accountPreference.ts";
import {
  settingsForNewChatRequest,
  settingsForNewChatAccount,
  type NewChatSettings,
} from "../../../app/projects/newChatSettings.ts";

type Props = {
  server?: StudioServer | undefined;
  serverServices?: ServerServices | undefined;
  projectId: string;
  path: string;
  onChange: (settings: Omit<NewChatSettings, "workspaceMode"> | null) => void;
};
// The parent can pass a new object for the same server. Reads restart only for another
// server identity, so a rebuilt list does not reset the user's account or models.
const serverIdentity = (server?: StudioServer) =>
  JSON.stringify([server?.id, server?.origin, server?.credentialId]);

export default function NewChatModelSettings(props: Props) {
  const inherited = useServerServices();
  const owner = props.server ?? inherited?.owner;
  const serverKey = serverIdentity(owner);
  const services = useMemo(() => {
    if (props.serverServices) return props.serverServices;
    if (!owner) return undefined;
    return inherited && serverIdentity(inherited.owner) === serverKey
      ? inherited
      : createServerServices(owner);
  }, [serverKey, inherited, props.serverServices]);
  useEffect(
    () => () => {
      if (
        services &&
        services !== inherited &&
        services !== props.serverServices
      )
        services.close();
    },
    [services, inherited, props.serverServices],
  );
  const content = (
    <ModelDraft
      key={JSON.stringify([serverKey, props.projectId, props.path])}
      {...props}
      server={owner}
    />
  );
  return services ? (
    <ServerServicesProvider services={services}>
      {content}
    </ServerServicesProvider>
  ) : (
    content
  );
}

const ModelDraft = observer(function ModelDraft({
  projectId,
  path,
  onChange,
}: Props) {
  const store = useServerStore();
  const sync = useServerSync();
  const [retryError, setRetryError] = useState("");
  const current = store.state().current;
  const state = resourceData(current);
  const accounts = state?.accountsSummary.get("current");
  const project = [...(state?.projects.values() || [])].find(
    (row) => row.id === projectId || row.path === path,
  );
  const [chosenAccountKey, setAccountKey] = useState("");
  const accountKey =
    chosenAccountKey ||
    (accounts
      ? newChatDefault(
          accounts.accounts,
          accounts.defaultAccountKey || "default",
          accounts.defaultAccountGroups,
          project,
          null,
        ).accountKey
      : "");
  const [workerExcludedAccountKeys, setWorkerExcludedAccountKeys] = useState<
    string[]
  >([]);
  const [reviewerExcludedAccountKeys, setReviewerExcludedAccountKeys] =
    useState<string[]>([]);
  const [settings, setSettings] = useState<
    Omit<NewChatSettings, "workspaceMode">
  >({});
  const error =
    "error" in current ? retryError || errorText(current.error) : "";
  const catalog = useWorkerModels(accountKey || "default", !!accounts);
  const provider = accounts?.accounts.find(
    (row) => row.id === accountKey,
  )?.provider;
  const allowed = useModelChoices();
  const availableModels = catalog.models.filter((row) =>
    isModelEnabled({ ...row, provider: row.provider || provider }, allowed),
  );
  // Draft-only preview uses the model catalog filtered by the person's choices.
  const model = settings.model || defaultChatModel(provider, availableModels);
  const modelInfo = modelForValue(catalog.models, model);
  // A Claude orchestrator that keeps the default model starts at the default effort.
  const defaultEffort =
    !settings.model &&
    settings.effort === undefined &&
    model === DEFAULT_CLAUDE_LEAD_MODEL
      ? DEFAULT_CLAUDE_LEAD_EFFORT
      : undefined;
  const worker = settings.worker_defaults;
  const defaultWorker = settingsFor({}, true);
  const agent: Agent = {
    id: "new-chat-draft",
    name: "New chat",
    isLead: true,
    source: GeneratedApiAgentSource.managed,
    status: GeneratedApiAgentStatus.idle,
    accountKey,
    provider,
    model,
    effort: settings.effort === undefined ? "medium" : settings.effort,
    fastMode: !!settings.fast_mode,
    daybreakEnabled: !!settings.daybreak_enabled,
    workerDefaults: {
      model: worker?.model === undefined ? defaultWorker.model : worker.model,
      effort:
        worker?.effort === undefined ? defaultWorker.effort : worker.effort,
      fastMode: !!worker?.fast_mode,
      daybreakEnabled: !!worker?.daybreak_enabled,
    },
    reviewDefaults: settings.review_defaults
      ? {
          model: settings.review_defaults.model,
          effort: settings.review_defaults.effort,
          ...(settings.review_defaults.fast_mode !== undefined
            ? { fastMode: settings.review_defaults.fast_mode }
            : {}),
        }
      : undefined,
  };
  const modelError =
    catalog.error ||
    (accounts && !catalog.loading && !model
      ? "No enabled models are available for this account. Select models in Studio settings → Models."
      : modelInfo &&
          !isModelEnabled(
            { ...modelInfo, provider: modelInfo.provider || provider },
            allowed,
          )
        ? "Choose a model enabled in Studio settings → Models."
        : "");
  const ready =
    !!accounts && !catalog.loading && !modelError && !!model && !error;
  useEffect(() => {
    onChange(
      ready
        ? {
            ...settingsForNewChatRequest(
              {
                ...settings,
                ...(defaultEffort ? { effort: defaultEffort } : {}),
              },
              accountKey,
              model,
            ),
            worker_excluded_account_keys: workerExcludedAccountKeys,
            reviewer_excluded_account_keys: reviewerExcludedAccountKeys,
          }
        : null,
    );
  }, [
    ready,
    settings,
    defaultEffort,
    accountKey,
    model,
    workerExcludedAccountKeys,
    reviewerExcludedAccountKeys,
    onChange,
  ]);
  const patch = useCallback(
    (
      value: Partial<
        import("../../../servers/net/api.ts").PostBody<"/api/conversation">
      >,
    ) => {
      const { id: _id, expected_account_key: _expected, ...values } = value;
      setSettings((previous) => ({ ...previous, ...values }));
    },
    [],
  );
  return (
    <Stack
      gap="lg"
      className="new-chat-model-settings"
      aria-label="Agent settings"
    >
      <ExecutionSettings
        input={{ kind: "draft", draft: agent }}
        catalog={catalog}
        accounts={accounts?.accounts || []}
        presentation="panel"
        settingsRow
        showAccountSummary
        workerExcludedAccountKeys={workerExcludedAccountKeys}
        reviewerExcludedAccountKeys={reviewerExcludedAccountKeys}
        onWorkerExcludedAccountKeysChange={setWorkerExcludedAccountKeys}
        onReviewerExcludedAccountKeysChange={setReviewerExcludedAccountKeys}
        onDraftChange={patch}
        onAccountChange={(key) => {
          setAccountKey(key);
          setSettings((previous) => settingsForNewChatAccount(previous, key));
        }}
      />
      {(error || modelError) && (
        <div role="alert">
          {error || modelError}
          <Button
            size="xs"
            onClick={() => {
              setRetryError("");
              if ("error" in current)
                void sync
                  .refreshState()
                  .catch((failure) => setRetryError(errorText(failure)));
              else catalog.retry();
            }}
          >
            Retry
          </Button>
        </div>
      )}
    </Stack>
  );
});

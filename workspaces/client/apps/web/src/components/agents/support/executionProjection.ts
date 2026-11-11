import { accountCanStart } from "../../../accountName.ts";
import type { PostBody, ServerApi } from "../../../api.ts";
import type { AppScope } from "../../../app/contracts.ts";
import { ProviderKind } from "../../../generated/providerKind.ts";
import { busy } from "../../../types.ts";
import {
  INHERITED_MODEL as DEFAULT,
  modelDisplayName,
  modelForValue,
  toModelOptions,
  type ModelOption,
} from "../../settings/models/support/modelOption.ts";
import type { AgentSetupPickerProps } from "../AgentSetupPicker/AgentSetupPicker.tsx";
import type { ScopedSettingsProps } from "../ExecutionSettings/ExecutionSettings.tsx";
import {
  isDaybreakAlias,
  supportsDaybreakMode,
  type ModelCatalogSource,
  type WorkerModelInfo,
  type useWorkerModels,
} from "./WorkerModelPicker.tsx";
import {
  accountOf,
  detailOf,
  executionScope,
  fastTier,
  queuedFor,
  reviewSettingsFor,
  settingsFor,
  summaryOf,
} from "./ExecutionSettingsStore.ts";

export const title = (value: string) =>
  value.charAt(0).toUpperCase() + value.slice(1);
export const infoFor = (catalog: Catalog, model: string) =>
  modelForValue(catalog.models, model);
/** Name a model row; a default alias shows the model it resolves to. */
export const displayModel = (catalog: Catalog, model: string) => {
  const info = infoFor(catalog, model);
  if (info?.isDefault && info.resolvedModel) {
    const target = catalog.models.find(
      (row) =>
        row.model !== info.model &&
        (row.model === info.resolvedModel ||
          row.resolvedModel === info.resolvedModel),
    );
    if (target) return modelDisplayName(target, target.model);
  }
  if (!info && model === "default") return "Auto";
  return modelDisplayName(info, model);
};

type ConversationBody = PostBody<"/api/conversation">;
type Catalog = ReturnType<typeof useWorkerModels>;
type Sources = {
  api: ServerApi;
  store: AppScope["store"];
  catalogReader: ModelCatalogSource;
  accountCatalog: Catalog;
  inheritedCatalog: Catalog;
  browse: [string | null, (value: string) => void];
};

/** Stateless projection of caller-owned props and computed server catalogs. */
export function projectExecution(props: ScopedSettingsProps, sources: Sources) {
  let {
    input,
    agentsUi: ui,
    section,
    readOnlyReason,
    catalog: parentCatalog,
    accounts = [],
    teamDefaults = false,
    onAccountChange,
    workerExcludedAccountKeys,
    reviewerExcludedAccountKeys,
    onWorkerExcludedAccountKeysChange,
    onReviewerExcludedAccountKeysChange,
    onDraftChange,
    role,
    acknowledged,
  } = props;
  const { api, accountCatalog, inheritedCatalog } = sources;
  const agent = summaryOf(input);
  const detail = detailOf(input);
  workerExcludedAccountKeys ??= onWorkerExcludedAccountKeysChange
    ? (detail?.workerExcludedAccountKeys ?? undefined)
    : undefined;
  reviewerExcludedAccountKeys ??= onReviewerExcludedAccountKeysChange
    ? (detail?.reviewerExcludedAccountKeys ?? undefined)
    : undefined;
  const { saved } = api;

  const canQueueSettings = detail?.nextTurnSettingsSupported === true;
  const formScope = executionScope(agent, role, section);
  const [saving] = ui.cell(formScope, "saving");
  const receiptKey = `next-turn-settings:${JSON.stringify([agent.id, accountOf(agent)])}`;
  const savedRequest =
    teamDefaults || onDraftChange
      ? null
      : saved<ConversationBody | null>(receiptKey, null);
  const [unconfirmed] = ui.cell(
    formScope,
    "unconfirmed",
    savedRequest?.id === agent.id && savedRequest.next_turn === true
      ? savedRequest
      : null,
  );
  const [pendingYolo] = ui.cell(formScope, "pendingYolo");
  const [error] = ui.cell(formScope, "error");
  const [status] = ui.cell(formScope, "status");
  const prefix = teamDefaults
    ? "Default subagent"
    : agent.isLead
      ? "Main agent"
      : "Subagent";
  const queued = queuedFor(agent, detail || {});
  const stored = settingsFor(agent, teamDefaults, detail || {});
  const reviewStored = reviewSettingsFor(detail || {});
  const reviewCurrent = acknowledged
    ? reviewSettingsFor(acknowledged)
    : reviewStored;
  const current = acknowledged
    ? settingsFor(acknowledged, teamDefaults)
    : stored;
  const effectiveAccount = teamDefaults ? null : accountOf(agent);
  const catalog = teamDefaults
    ? accountCatalog
    : readOnlyReason
      ? inheritedCatalog
      : parentCatalog;
  const connectedAccounts = accounts.filter((account) =>
    workerExcludedAccountKeys !== undefined ||
    reviewerExcludedAccountKeys !== undefined ||
    role === "review"
      ? true
      : teamDefaults
        ? accountCanStart(account)
        : agent.accountDestinations?.some(
            (destination) => destination.accountKey === account.id,
          ),
  );
  const activeAccount = accounts.find(
    (account) => account.id === (effectiveAccount || accountOf(agent)),
  );
  const automaticProvider =
    infoFor(catalog, current.model || agent.model || "")?.provider ||
    activeAccount?.provider ||
    agent.provider ||
    "";
  // The main agent's provider: "Same as main agent" exists only on its tab.
  const leadProvider =
    accounts.find((account) => account.id === accountOf(agent))?.provider ||
    agent.provider ||
    "";
  const [browsedProvider, setBrowsedProvider] = sources.browse;
  const pickerProvider =
    role === "review"
      ? browsedProvider || ProviderKind.Codex
      : browsedProvider ||
        (teamDefaults ? automaticProvider : activeAccount?.provider) ||
        automaticProvider ||
        "";
  const pickerProviders = [
    ...new Set(connectedAccounts.map((account) => account.provider)),
  ];
  const chooseAccount = (key: string | null) => {
    if (!teamDefaults && key) onAccountChange?.(key);
  };

  const selectedModel = current.model || agent.model || "";
  const info = infoFor(catalog, selectedModel);
  const selectedProvider = teamDefaults ? info?.provider : agent.provider;
  const active =
    !teamDefaults && (!!agent.inFlight || busy.has(agent.status ?? ""));
  const catalogKnown = !catalog.loading && !catalog.error;
  const disabled =
    !detail ||
    !!readOnlyReason ||
    saving ||
    !!unconfirmed ||
    (active && !canQueueSettings) ||
    (!teamDefaults && !catalogKnown);
  const models = catalog.models.filter((row) => !isDaybreakAlias(row.model));
  const daybreakEnabled = !!current.daybreak_enabled;
  const supportsMode = (
    row: WorkerModelInfo | undefined,
    enabled = daybreakEnabled,
  ) => supportsDaybreakMode(row, enabled);
  const modeSupported = supportsMode(info);
  const modeQueued =
    !teamDefaults &&
    !!(active || queued || unconfirmed) &&
    daybreakEnabled !== !!detail?.daybreakEnabled;
  const modeLabel = daybreakEnabled ? "Daybreak" : "Standard";
  const modelOptions = toModelOptions(models, {
    provider: selectedProvider || "",
    disabledReason: (row) =>
      supportsMode(row)
        ? undefined
        : `Unavailable in ${modeLabel.toLowerCase()} mode.`,
  });
  if (teamDefaults) {
    const leadModel = agent.model ?? "";
    const leadInfo = infoFor(catalog, leadModel);
    modelOptions.unshift({
      value: DEFAULT,
      label: `Same as main agent (${modelDisplayName(leadInfo, leadModel)})`,
      provider: leadProvider || "",
      badges: [],
      description: leadInfo?.description || undefined,
      disabledReason: supportsMode(leadInfo)
        ? undefined
        : `Unavailable in ${modeLabel.toLowerCase()} mode.`,
    });
  }
  if (!modelForValue(modelOptions, current.model || DEFAULT))
    modelOptions.unshift({
      value: current.model || DEFAULT,
      label: modelDisplayName(selectedModel),
      provider: selectedProvider || "",
      badges: [],
      disabledReason: "Unavailable on this account.",
    });
  const reviewModels = catalog.models.filter(
    (row) =>
      row.model.startsWith("gpt-") &&
      !isDaybreakAlias(row.model) &&
      supportsDaybreakMode(row, false),
  );
  const reviewInfo = modelForValue(reviewModels, reviewCurrent.model);
  const reviewFast = reviewCurrent.fast_mode ?? !!detail?.fastMode;
  const reviewOptions = toModelOptions(reviewModels);
  if (reviewCurrent.model && !modelForValue(reviewOptions, reviewCurrent.model))
    reviewOptions.push({
      value: reviewCurrent.model,
      label: modelDisplayName(reviewCurrent.model),
      provider: ProviderKind.Codex,
      badges: [],
      disabledReason: "Unavailable for review.",
    });
  const shownTransfer = teamDefaults
    ? undefined
    : agent.accountTransferProgress;
  const blockedTransfer = shownTransfer?.kind === "blocked";
  const shownTransferTarget =
    shownTransfer && "targetAccountKey" in shownTransfer
      ? accounts.find(
          (account) => account.id === shownTransfer.targetAccountKey,
        )
      : undefined;
  const selectedOptions = role === "review" ? reviewOptions : modelOptions;
  const storedPickerModel =
    role === "review" ? reviewCurrent.model || "" : current.model || DEFAULT;
  const pickerModel =
    modelForValue(selectedOptions, storedPickerModel)?.value ||
    storedPickerModel;
  const pickerModels = selectedOptions
    .filter((row) => {
      if (row.value === DEFAULT)
        return role === "worker" && leadProvider === pickerProvider;
      const model = infoFor(catalog, row.value);
      if (!model)
        return !!row.disabledReason && row.provider === pickerProvider;
      return (
        !model.hidden &&
        (role === "review" ||
          (model.provider ||
            activeAccount?.provider ||
            agent.provider ||
            "") === pickerProvider)
      );
    })
    .map((row) => ({
      ...row,
      label:
        row.value === DEFAULT
          ? "Same as main agent"
          : displayModel(catalog, row.value),
    }));
  const pickerInfo = role === "review" ? reviewInfo : info;
  const pickerEfforts = (pickerInfo?.supportedReasoningEfforts || []).map(
    (row) => ({
      value: row.reasoningEffort,
      label: title(row.reasoningEffort),
    }),
  );
  const selectedEffort =
    (role === "review" ? reviewCurrent.effort : current.effort) ||
    pickerInfo?.defaultReasoningEffort ||
    pickerEfforts[0]?.value ||
    "";
  return {
    props,
    sources,
    formScope,
    reviewModels,
    reviewCurrent,
    workerExcludedAccountKeys,
    reviewerExcludedAccountKeys,
    agent,
    current,
    saving,
    pickerProvider,
    pickerProviders,
    setBrowsedProvider,
    connectedAccounts,
    chooseAccount,
    pickerModels: uniqueModelLabels(pickerModels, pickerModel),
    pickerModel,
    prefix,
    pickerEfforts,
    selectedEffort,
    modeSupported,
    disabled,
    catalog,
    reviewInfo,
    info,
    reviewFast,
    selectedProvider,
    daybreakEnabled,
    supportsMode,
    blockedTransfer,
    shownTransfer,
    shownTransferTarget,
    error,
    status,
    queued,
    active,
    canQueueSettings,
    modeQueued,
    detail,
    modeLabel,
    unconfirmed,
    pendingYolo,
  };
}

/** Keep the selected alias, then the recommended alias, preserving row order. */
export function uniqueModelLabels(rows: ModelOption[], selected: string) {
  const labels = new Map<string, ModelOption>();
  for (const row of rows) {
    const previous = labels.get(row.label);
    if (
      !previous ||
      (previous.value !== selected &&
        (row.value === selected ||
          (!previous.badges.includes("default") &&
            row.badges.includes("default"))))
    )
      labels.set(row.label, row);
  }
  return rows.filter((row) => labels.get(row.label) === row);
}

export function executionPicker(
  p: ScopedSettingsProps,
  view: ReturnType<typeof projectExecution>,
): AgentSetupPickerProps {
  const {
    onDraftChange,
    role,
    onRole,
    workersHidden = false,
    roleSwitchId,
    readOnlyReason,
    inheritedUnavailable,
    teamDefaults,
    accountDisabled = false,
    onAccountChange,
    presentation,
    onWorkerExcludedAccountKeysChange,
    onReviewerExcludedAccountKeysChange,
  } = p;
  const execution = p.agentsUi.execution;
  return {
    retainCurrentModel: !onDraftChange,
    role: role,
    onRole: onRole,
    workersHidden,
    roleSwitchId: roleSwitchId,
    readOnlyReason: readOnlyReason,
    provider: view.pickerProvider,
    capabilityProvider:
      role === "review" ? ProviderKind.Codex : view.agent.provider || undefined,
    providers: view.pickerProviders,
    onProvider: view.setBrowsedProvider,
    accounts: inheritedUnavailable ? [] : view.connectedAccounts,
    accountUnavailableReason:
      role === "review"
        ? (account) =>
            account.provider !== ProviderKind.Codex
              ? "Review requires a Codex account."
              : !accountCanStart(account)
                ? "This account is unavailable."
                : undefined
        : teamDefaults
          ? undefined
          : (account) => {
              const destination = view.agent.accountDestinations?.find(
                (entry) => entry.accountKey === account.id,
              );
              return destination?.eligibility.kind === "unavailable"
                ? destination.eligibility.reason
                : undefined;
            },
    accountKey: accountOf(view.agent),
    onAccount: view.chooseAccount,
    excludedKeys:
      role === "worker"
        ? view.workerExcludedAccountKeys
        : role === "review"
          ? view.reviewerExcludedAccountKeys
          : undefined,
    onExcludedKeysChange:
      role === "worker"
        ? onWorkerExcludedAccountKeysChange
        : role === "review"
          ? onReviewerExcludedAccountKeysChange
          : undefined,
    showAccountChooser: !teamDefaults,
    accountDisabled:
      !!readOnlyReason ||
      view.saving ||
      accountDisabled ||
      (!teamDefaults && !onAccountChange),
    modelId:
      presentation !== "menu" ? `${view.agent.id}-${role}-model` : undefined,
    models: inheritedUnavailable
      ? [
          {
            value: "unavailable",
            label: "Inherited settings unavailable",
            provider: view.pickerProvider,
            badges: [],
          },
        ]
      : view.pickerModels,
    model: inheritedUnavailable ? "unavailable" : view.pickerModel,
    modelLabel: role === "review" ? "Review model" : view.prefix + " model",
    onModel: (value) => {
      if (role === "review") {
        if (value && value !== DEFAULT)
          void execution.changeReview(view, { model: value });
      } else
        void execution.change(view, {
          model: value === DEFAULT ? null : value,
        });
    },
    efforts: inheritedUnavailable ? [] : view.pickerEfforts,
    effort: view.selectedEffort,
    effortLabel:
      role === "review"
        ? "Default review reasoning"
        : view.prefix + " reasoning",
    onEffort: (value) => {
      if (role === "review")
        void execution.changeReview(view, { effort: value });
      else void execution.change(view, { effort: value });
    },
    effortDisabled:
      role === "review" ? !view.reviewCurrent.model : !view.modeSupported,
    disabled: view.disabled || view.catalog.loading || !!view.catalog.error,
    fastAvailable:
      !inheritedUnavailable &&
      !!fastTier(role === "review" ? view.reviewInfo : view.info),
    fast: role === "review" ? view.reviewFast : view.current.fast_mode,
    onFast: () => {
      if (role === "review")
        void execution.changeReview(view, { fast_mode: !view.reviewFast });
      else void execution.change(view, { fast_mode: !view.current.fast_mode });
    },
    daybreakAvailable:
      !inheritedUnavailable &&
      view.selectedProvider === ProviderKind.Codex &&
      (view.daybreakEnabled || view.supportsMode(view.info, true)),
    daybreak: view.daybreakEnabled,
    onDaybreak: () =>
      void execution.change(view, {
        daybreak_enabled: !view.daybreakEnabled,
      }),
  };
}

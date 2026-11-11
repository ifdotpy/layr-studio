import {
  workerCatalogKey,
  loadingCatalog,
} from "../../agents/support/WorkerModelPicker.tsx";
import type { ServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { makeAutoObservable } from "mobx";
import type { ServerServices } from "../../../servers/serverServices.ts";
import {
  AgentSource as GeneratedApiAgentSource,
  AgentStatus as GeneratedApiAgentStatus,
} from "../../../generated/api.ts";
import {
  isModelEnabled,
  readModelChoices,
} from "../models/support/modelChoices.ts";
import { modelForValue } from "../models/support/modelOption.ts";
import { errorText } from "../../../api.ts";
import { resourceData } from "../../../store/runtime.ts";
import type { Agent } from "../../../types.ts";
import { settingsFor } from "../../agents/ExecutionSettings/ExecutionSettings.tsx";
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

export class NewChatModelStore {
  chosenAccountKey = "";
  workerExcludedAccountKeys: string[] = [];
  reviewerExcludedAccountKeys: string[] = [];
  settings: Omit<NewChatSettings, "workspaceMode"> = {};
  retryError = "";
  readonly areas;
  get agentsUi() {
    return this.areas.agentsUi;
  }
  readonly services;
  readonly projectId;
  readonly path;
  constructor(
    services: ServerServices,
    projectId: string,
    path: string,
    areas: ServerAreas,
  ) {
    this.areas = areas;
    this.services = services;
    this.projectId = projectId;
    this.path = path;
    makeAutoObservable(
      this,
      { services: false, areas: false },
      { autoBind: true },
    );
  }
  get view() {
    const current = this.services.store.state().current;
    const state = resourceData(current);
    const accounts = state?.accountsSummary.get("current");
    const project = [...(state?.projects.values() || [])].find(
      (row) => row.id === this.projectId || row.path === this.path,
    );
    const accountKey =
      this.chosenAccountKey ||
      (accounts
        ? newChatDefault(
            accounts.accounts,
            accounts.defaultAccountKey || "default",
            accounts.defaultAccountGroups,
            project,
            null,
          ).accountKey
        : "");
    const error =
      "error" in current ? this.retryError || errorText(current.error) : "";
    const catalog =
      this.agentsUi.workerCatalogs.get(
        workerCatalogKey(accountKey || "default"),
      ) ?? loadingCatalog;
    const provider = accounts?.accounts.find(
      (row) => row.id === accountKey,
    )?.provider;
    const allowed = readModelChoices(this.services.storage);
    const availableModels = catalog.models.filter((row) =>
      isModelEnabled({ ...row, provider: row.provider || provider }, allowed),
    );
    // Draft-only preview uses the model catalog filtered by the person's choices.
    const model =
      this.settings.model || defaultChatModel(provider, availableModels);
    const modelInfo = modelForValue(catalog.models, model);
    // A Claude orchestrator that keeps the default model starts at the default effort.
    const defaultEffort =
      !this.settings.model &&
      this.settings.effort === undefined &&
      model === DEFAULT_CLAUDE_LEAD_MODEL
        ? DEFAULT_CLAUDE_LEAD_EFFORT
        : undefined;
    const worker = this.settings.worker_defaults;
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
      effort:
        this.settings.effort === undefined ? "medium" : this.settings.effort,
      fastMode: !!this.settings.fast_mode,
      daybreakEnabled: !!this.settings.daybreak_enabled,
      workerDefaults: {
        model: worker?.model === undefined ? defaultWorker.model : worker.model,
        effort:
          worker?.effort === undefined ? defaultWorker.effort : worker.effort,
        fastMode: !!worker?.fast_mode,
        daybreakEnabled: !!worker?.daybreak_enabled,
      },
      reviewDefaults: this.settings.review_defaults
        ? {
            model: this.settings.review_defaults.model,
            effort: this.settings.review_defaults.effort,
            fastMode: this.settings.review_defaults.fast_mode,
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

    return {
      current,
      accounts,
      accountKey,
      catalog,
      agent,
      error,
      modelError,
      selection: ready
        ? {
            ...settingsForNewChatRequest(
              {
                ...this.settings,
                ...(defaultEffort ? { effort: defaultEffort } : {}),
              },
              accountKey,
              model,
            ),
            worker_excluded_account_keys: this.workerExcludedAccountKeys,
            reviewer_excluded_account_keys: this.reviewerExcludedAccountKeys,
          }
        : null,
    };
  }
  get selection() {
    return this.view.selection;
  }
  patch(
    value: Partial<
      import("../../../servers/net/api.ts").PostBody<"/api/conversation">
    >,
  ) {
    const { id: _id, expected_account_key: _expected, ...values } = value;
    this.settings = { ...this.settings, ...values };
  }
  setAccountKey(key: string) {
    this.chosenAccountKey = key;
    this.settings = settingsForNewChatAccount(this.settings, key);
  }
  setWorkerExcluded(keys: string[]) {
    this.workerExcludedAccountKeys = keys;
  }
  setReviewerExcluded(keys: string[]) {
    this.reviewerExcludedAccountKeys = keys;
  }
  *retry() {
    this.retryError = "";
    if (!("error" in this.view.current)) {
      this.view.catalog.retry();
      return;
    }
    try {
      yield this.services.sync.refreshState();
    } catch (error) {
      this.retryError = errorText(error);
    }
  }
}

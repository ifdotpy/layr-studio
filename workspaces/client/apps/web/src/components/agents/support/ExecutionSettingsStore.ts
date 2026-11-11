import { flowValue as resolved } from "../../../flowValue.ts";
import type { ResourceVersion } from "../../../sync/subscriptions/contracts.ts";
import { ApiSchemaMismatchError } from "../../../api.ts";
import { serverMutationResponseIsUncertain } from "../../../servers/mutationIdentity.ts";
import { flowResult, makeAutoObservable } from "mobx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import type { ServerApi } from "../../../api.ts";
import { ApiError, errorText, type PostBody } from "../../../api.ts";
import {
  clearSettingsRequest,
  settingsRequestId,
} from "../../../app/settings/settingsRequestId.ts";
import { appendAppLog } from "../../../appLog.ts";
import { AgentPermissionMode } from "../../../generated/api.ts";
import { agentId } from "../../../generated/observableResources.ts";
import { ProviderKind } from "../../../generated/providerKind.ts";
import { resourceData } from "../../../store/runtime.ts";
import type { Agent } from "../../../types.ts";
import {
  INHERITED_MODEL as DEFAULT,
  modelDisplayName,
  modelForValue,
} from "../../settings/models/support/modelOption.ts";
import type { SetupRole } from "../AgentSetupPicker/AgentSetupPicker.tsx";
import type { AgentsUi } from "../support/AgentsUi.ts";
import {
  readWorkerModels,
  supportsDaybreakMode,
  type WorkerModelInfo,
} from "../support/WorkerModelPicker.tsx";
import type { projectExecution } from "./executionProjection.ts";

type Body = PostBody<"/api/conversation">;
export type SettingsValue = {
  model: string | null;
  effort: string | null;
  fast_mode: boolean;
  daybreak_enabled: boolean;
  delegation_depth: number;
};
export type ReviewSettings = NonNullable<Body["review_defaults"]>;
type ExecutionContext = ReturnType<typeof projectExecution>;
export const fastTier = (info?: WorkerModelInfo) =>
  info?.serviceTiers?.find((tier) => tier.id === "priority");

/** One server owns execution edits, permission receipts and transfer actions. */
export class ExecutionSettingsStore {
  readonly ui;
  constructor(ui: AgentsUi) {
    this.ui = ui;
    makeAutoObservable(this, { ui: false }, { autoBind: true });
  }
  *submit(c: ExecutionContext, request: Body, notice = "") {
    if (c.props.readOnlyReason) return;
    if (c.props.onDraftChange && request.yolo_mode === undefined) {
      c.props.onDraftChange(
        request.review_defaults
          ? { review_defaults: request.review_defaults }
          : request,
      );
      return;
    }
    const { ui } = this;
    const key = c.formScope;
    if (ui.fields.get(key)?.saving) return;
    const accountKey = accountOf(c.agent);
    const receiptKey = `next-turn-settings:${JSON.stringify([c.agent.id, accountKey])}`;
    const permissions = request.yolo_mode !== undefined;
    const live =
      !permissions && !c.props.teamDefaults && !request.review_defaults;
    const accept = permissions
      ? undefined
      : c.props.beginSettingsSave(
          request.review_defaults
            ? "review"
            : c.props.teamDefaults
              ? "worker"
              : "orchestrator",
        );
    request = { ...request, expected_account_key: accountKey };
    const statusKey = request.request_id || randomUUID();
    if (live)
      c.props.onSettingsChange({
        agentId: c.agent.id,
        accountKey,
        requestKey: statusKey,
        phase: "saving",
        before: {
          model: c.current.model,
          effort: c.current.effort,
          fastMode: c.current.fast_mode,
        },
        requested: {
          model: request.model ?? null,
          effort: request.effort ?? null,
          fastMode: !!request.fast_mode,
        },
      });
    ui.set(key, "saving", true);
    ui.set(key, "error", "");
    if (!permissions) ui.set(key, "status", { kind: "saving", text: "" });
    if (request.next_turn) {
      c.sources.api.save(receiptKey, request);
      ui.set(key, "unconfirmed", request);
    }
    const clearReceipt = () => {
      if (
        c.sources.api.saved<Body | null>(receiptKey, null)?.request_id ===
        request.request_id
      )
        c.sources.api.save(receiptKey, null);
      ui.set(key, "unconfirmed", null);
    };
    let confirmed = false;
    try {
      const defaults = request.worker_defaults;
      const model = defaults?.model ?? null;
      if (defaults && model !== c.current.model) {
        const offered = yield* resolved(
          readWorkerModels(c.sources.catalogReader, accountKey, true),
        );
        const selected = modelForValue(offered, model || c.agent.model);
        if (!selected || selected.hidden)
          throw new Error(
            "This model is not available for the selected worker accounts.",
          );
      }
      const canonical = yield* resolved(
        permissions
          ? c.sources.api.post("/api/conversation", request)
          : c.sources.api.post(
              "/api/conversation",
              request,
              request.next_turn ||
                c.props.teamDefaults ||
                request.review_defaults
                ? { timeoutMs: 15000 }
                : {},
            ),
      );

      if (
        !canonical ||
        canonical.id !== c.agent.id ||
        accountOf(canonical) !== accountKey
      )
        throw new Error(
          "The settings account changed. Check the current settings.",
        );
      confirmed = true;
      if (live)
        c.props.onSettingsConfirmed(
          statusKey,
          request.next_turn === true,
          canonical,
        );
      if (request.next_turn) clearReceipt();
      if (permissions) {
        clearSettingsRequest(
          c.sources.api,
          `permissions:${c.agent.id}:${accountKey}`,
          request.request_id || statusKey,
        );
        ui.set(key, "pendingYolo", {
          read: () =>
            resourceData(
              c.sources.store.agentDetail(agentId(c.agent.id)).current,
            )?.permissionMode === AgentPermissionMode.bypassPermissions,
          value:
            canonical.permissionMode === AgentPermissionMode.bypassPermissions,
          baseline:
            c.detail?.permissionMode === AgentPermissionMode.bypassPermissions,
        });
      } else {
        accept?.(canonical);
        ui.set(key, "status", { kind: "saved", text: notice });
      }
    } catch (failure) {
      const rejected =
        failure instanceof ApiError &&
        [400, 401, 403, 404, 409].includes(failure.status);
      if (request.next_turn && !confirmed && rejected) clearReceipt();
      if (live && !confirmed) {
        const uncertain = !!request.next_turn && !rejected;
        appendAppLog({
          at: Date.now(),
          level: "error",
          title: uncertain
            ? "Execution settings save unconfirmed"
            : "Execution settings change failed",
          reason: errorText(failure),
          detail: `Agent: ${c.agent.id}; account: ${accountKey}; request: ${statusKey}`,
        });
        c.props.onSettingsFailure(statusKey, errorText(failure), uncertain);
      }
      if (!permissions) ui.set(key, "status", null);
      ui.set(
        key,
        "error",
        confirmed
          ? `Settings saved. ${errorText(failure)}`
          : errorText(failure),
      );
    } finally {
      ui.set(key, "saving", false);
    }
  }
  change(c: ExecutionContext, patch: Partial<SettingsValue>) {
    if (
      c.disabled ||
      ("daybreak_enabled" in patch && c.selectedProvider !== ProviderKind.Codex)
    )
      return;
    const next = { ...c.current, ...patch };
    const known = !c.catalog.loading && !c.catalog.error;
    const adjustments: string[] = [];
    const info = () =>
      modelForValue(c.catalog.models, next.model || c.agent.model || "");
    if (
      "daybreak_enabled" in patch &&
      known &&
      !supportsDaybreakMode(info(), next.daybreak_enabled)
    ) {
      const eligible = c.catalog.models.filter((row) =>
        supportsDaybreakMode(row, next.daybreak_enabled),
      );
      const replacement = eligible.find((row) => row.isDefault) || eligible[0];
      if (!replacement) {
        this.ui.set(
          c.formScope,
          "error",
          "No model supports this mode for the selected account.",
        );
        return;
      }
      next.model = replacement.model;
      adjustments.push(
        `Model changed to ${modelDisplayName(next.model)} for ${next.daybreak_enabled ? "Daybreak" : "standard mode"}.`,
      );
    }
    if (known && !supportsDaybreakMode(info(), next.daybreak_enabled)) {
      this.ui.set(
        c.formScope,
        "error",
        "Choose a model that supports this mode for the selected account.",
      );
      return;
    }
    if (known && ("model" in patch || next.model !== c.current.model)) {
      const nextInfo = info();
      if (
        !nextInfo?.supportedReasoningEfforts?.some(
          (row) => row.reasoningEffort === next.effort,
        )
      ) {
        if (next.effort)
          adjustments.push("Reasoning changed to the model default.");
        next.effort = null;
      }
      if (!fastTier(nextInfo)) {
        if (next.fast_mode)
          adjustments.push("Fast mode is unavailable and was turned off.");
        next.fast_mode = false;
      }
    }
    return flowResult(
      this.submit(
        c,
        c.props.teamDefaults
          ? { id: c.agent.id, worker_defaults: next }
          : {
              id: c.agent.id,
              model: next.model,
              effort: next.effort,
              fast_mode: next.fast_mode,
              daybreak_enabled: next.daybreak_enabled,
              ...(c.active || c.queued
                ? { next_turn: true, request_id: randomUUID() }
                : {}),
            },
        adjustments.join(" "),
      ),
    );
  }
  changeReview(c: ExecutionContext, patch: Partial<ReviewSettings>) {
    if (c.disabled || this.ui.fields.get(c.formScope)?.saving) return;
    const next = { ...c.reviewCurrent, ...patch };
    if (!next.model) {
      this.ui.set(c.formScope, "error", "Choose a review model");
      return;
    }
    if ("model" in patch) {
      next.effort = null;
      if (
        !fastTier(modelForValue(c.reviewModels, next.model)) &&
        (next.fast_mode ?? c.detail?.fastMode)
      )
        next.fast_mode = false;
    }
    return flowResult(
      this.submit(c, { id: c.agent.id, review_defaults: next }),
    );
  }
  changeYolo(c: ExecutionContext, enabled: boolean) {
    if (
      this.ui.fields.get(c.formScope)?.saving ||
      !c.agent.isLead ||
      c.agent.provider !== ProviderKind.Codex
    )
      return;
    try {
      const requestId = settingsRequestId(
        c.sources.api,
        `permissions:${c.agent.id}:${accountOf(c.agent)}`,
        enabled,
      );
      return flowResult(
        this.submit(c, {
          id: c.agent.id,
          request_id: requestId,
          yolo_mode: enabled,
        }),
      );
    } catch (failure) {
      this.ui.set(c.formScope, "error", errorText(failure));
      return;
    }
  }
  *saveExclusions(
    id: string,
    baseline: Readonly<ResourceVersion> | null | undefined,
    field: "worker_excluded_account_keys" | "reviewer_excluded_account_keys",
    keys: string[],
    post: ServerApi["post"],
    notify: (message: string) => void,
  ) {
    const scope = `exclusions:${id}`;
    this.ui.set(scope, "exclusionsSaving", baseline);
    try {
      yield post("/api/conversation", { id, [field]: keys });
    } catch (error) {
      notify(errorText(error));
      if (
        error instanceof ApiError &&
        (!(error instanceof ApiSchemaMismatchError) || error.markedResponse) &&
        !serverMutationResponseIsUncertain(error.status, error.response)
      )
        this.ui.set(scope, "exclusionsSaving", (pending) =>
          pending === baseline ? null : pending,
        );
    }
  }
  *transfer(c: ExecutionContext, action: "retry" | "cancel") {
    const transfer = c.props.teamDefaults
      ? undefined
      : c.agent.accountTransferProgress;
    if (
      this.ui.fields.get(c.formScope)?.saving ||
      !transfer ||
      !("actions" in transfer) ||
      !transfer.actions.includes(action)
    )
      return;
    this.ui.set(c.formScope, "saving", true);
    this.ui.set(c.formScope, "error", "");
    try {
      yield c.sources.api.post("/api/agents/account-transfer", {
        action,
        request_id: transfer.id,
      });
    } catch (failure) {
      appendAppLog({
        at: Date.now(),
        level: "error",
        title: `Account transfer ${action} failed`,
        reason: errorText(failure),
      });
      this.ui.set(c.formScope, "error", errorText(failure));
    } finally {
      this.ui.set(c.formScope, "saving", false);
    }
  }
}

type SettingsAgent = Pick<
  Agent,
  | "accountKey"
  | "pendingSettingsAccountKey"
  | "pendingSettings"
  | "workerDefaults"
  | "model"
  | "effort"
  | "fastMode"
  | "daybreakEnabled"
>;
export type AgentSettingsInput = {
  kind: "agent";
  summary: Agent;
  detail: Agent | undefined;
};
export type ExecutionSettingsInput =
  | AgentSettingsInput
  | { kind: "draft"; draft: Agent };
export const summaryOf = (input: ExecutionSettingsInput) =>
  input.kind === "agent" ? input.summary : input.draft;
export const detailOf = (input: ExecutionSettingsInput) =>
  input.kind === "agent" ? input.detail : input.draft;

export const accountOf = (agent: Pick<Agent, "accountKey">) =>
  agent.accountKey || "default";
export const queuedFor = (
  agent: SettingsAgent,
  detail: SettingsAgent = agent,
) =>
  detail.pendingSettingsAccountKey &&
  detail.pendingSettingsAccountKey !== accountOf(agent)
    ? null
    : detail.pendingSettings;
export const settingsFor = (
  agent: SettingsAgent,
  teamDefaults: boolean,
  detail: SettingsAgent = agent,
): SettingsValue => {
  if (teamDefaults)
    return {
      model:
        detail.workerDefaults?.model === undefined
          ? "gpt-6-luna"
          : (detail.workerDefaults.model ?? null),
      effort:
        detail.workerDefaults?.effort === undefined
          ? "high"
          : detail.workerDefaults.effort,
      fast_mode: !!detail.workerDefaults?.fastMode,
      daybreak_enabled: !!detail.workerDefaults?.daybreakEnabled,
      delegation_depth: detail.workerDefaults?.delegationDepth ?? 1,
    };
  const queued = queuedFor(agent, detail);
  return {
    model: queued?.model ?? detail.model ?? agent.model ?? null,
    effort: queued ? (queued.effort ?? null) : (detail.effort ?? null),
    fast_mode: queued ? !!queued.fastMode : !!detail.fastMode,
    daybreak_enabled: !!(queued?.daybreakEnabled ?? detail.daybreakEnabled),
    delegation_depth: 1,
  };
};
export const reviewSettingsFor = (
  agent: Pick<Agent, "reviewDefaults">,
): ReviewSettings => ({
  model:
    agent.reviewDefaults?.model === DEFAULT
      ? null
      : (agent.reviewDefaults?.model ?? null),
  effort: agent.reviewDefaults?.effort ?? null,
  ...(agent.reviewDefaults?.fastMode != null
    ? { fast_mode: agent.reviewDefaults.fastMode }
    : {}),
});
export const settingsLabel = (
  agent: Agent,
  role: SetupRole,
  teamDefaults: boolean,
) =>
  role === "review"
    ? "Review settings"
    : teamDefaults
      ? "Subagent defaults"
      : agent.isLead
        ? "Main agent settings"
        : "Subagent settings";

export const executionScope = (
  agent: Agent,
  role: SetupRole,
  section = "model",
) => JSON.stringify([agent.id, accountOf(agent), role, section]);

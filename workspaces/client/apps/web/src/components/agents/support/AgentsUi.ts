import { ExecutionSettingsStore } from "./ExecutionSettingsStore.ts";
import { createAgentCatalog } from "./state.ts";
import { createWorkerCatalog, workerCatalogKey } from "./WorkerModelPicker.tsx";
import { appendAppLog } from "../../../appLog.ts";
import {
  makeAutoObservable,
  observable,
  reaction,
  autorun,
  action,
  actionBound,
  compareStructural,
} from "mobx";
import type { SetupRole } from "../AgentSetupPicker/AgentSetupPicker.tsx";
import type { Agent } from "../../../types.ts";
import type { PostBody } from "../../../api.ts";
import {
  observeSettingsChange,
  type SettingsChange,
} from "../ExecutionSettings/ExecutionSettings.tsx";
import type { ResourceVersion } from "../../../sync/subscriptions/contracts.ts";
import type { AppScope } from "../../../app/contracts.ts";
import { agentId } from "../../../generated/observableResources.ts";
import { resourceData } from "../../../store/runtime.ts";
import { ConcurrencyStore } from "../SubagentConcurrencyControl/SubagentConcurrencyControl.tsx";

type Body = PostBody<"/api/conversation">;
type Receipt = {
  value: Agent;
  baseline: string;
  read: () => string;
};
export type Fields = {
  opened: boolean;
  role: SetupRole | null;
  changeStatus: SettingsChange | null;
  confirmed: Partial<Record<SetupRole, Receipt>>;
  saving: boolean;
  unconfirmed: Body | null;
  pendingYolo: {
    value: boolean;
    baseline: boolean;
    read: () => boolean;
  } | null;
  error: string;
  status: { kind: "saving" | "saved"; text: string } | null;
  exclusionsSaving: Readonly<ResourceVersion> | null | undefined;
};
const defaults: Fields = {
  opened: false,
  role: null,
  changeStatus: null,
  confirmed: {},
  saving: false,
  unconfirmed: null,
  pendingYolo: null,
  error: "",
  status: null,
  exclusionsSaving: null,
};

/** Business edits and receipt lifetimes belong to the authenticated server. */
export class AgentsUi {
  readonly execution = new ExecutionSettingsStore(this);
  readonly agentCatalog;
  readonly workerCatalogs = observable.map<
    string,
    ReturnType<typeof createWorkerCatalog>
  >(undefined, { deep: false });
  fields = observable.map<string, Fields>(undefined, { deep: false });
  concurrency = observable.map<string, ConcurrencyStore>();
  private stops: Array<() => void>;
  readonly scope: AppScope | undefined;
  constructor(
    scope?: AppScope,
    workspaceId: () => string = () => "",
    selected: () => string | undefined = () => undefined,
  ) {
    this.scope = scope;
    this.agentCatalog = scope ? createAgentCatalog(scope.store) : undefined;
    makeAutoObservable(
      this,
      {
        scope: false,
        execution: false,
        agentCatalog: false,
        workerCatalogs: false,
        cell: false,
        fields: false,
        concurrency: false,
        set: action,
        control: action,
        clearReceipts: actionBound,
      },
      { autoBind: true },
    );
    this.stops = [
      autorun(() => {
        for (const [key, state] of this.fields) {
          const permission = state.pendingYolo;
          if (
            permission &&
            !state.saving &&
            (permission.value === permission.read() ||
              permission.baseline !== permission.read())
          )
            this.set(key, "pendingYolo", null);
          for (const role of ["orchestrator", "worker", "review"] as const) {
            const receipt = state.confirmed[role];
            if (receipt && receipt.baseline !== receipt.read())
              this.set(key, "confirmed", (old) => {
                const { [role]: _stale, ...confirmed } = old;
                return confirmed;
              });
          }
        }
      }),
    ];
    if (!scope) return;
    this.stops.push(
      reaction(
        () => {
          const state = resourceData(scope.store.state().current);
          const keys = new Set([
            "default",
            ...(state?.accountsSummary
              .get("current")
              ?.accounts.map((account) => account.id) ?? []),
            ...[...(state?.agents.values() ?? [])].map(
              (agent) => agent.accountKey || "default",
            ),
          ]);
          return {
            catalogs: [...keys].sort().flatMap((accountKey) =>
              [false, true].map((workers) => ({
                accountKey,
                workers,
                key: workerCatalogKey(accountKey, workers),
              })),
            ),
            pending: [...this.workerCatalogs]
              .filter(([, catalog]) => catalog.pending)
              .map(([key]) => key),
          };
        },
        ({ catalogs }) => {
          const active = new Set(catalogs.map((catalog) => catalog.key));
          for (const { key, accountKey, workers } of catalogs)
            if (!this.workerCatalogs.has(key))
              this.workerCatalogs.set(
                key,
                createWorkerCatalog(
                  scope.store,
                  scope.services.modelReader,
                  accountKey,
                  workers,
                ),
              );
          for (const [key, catalog] of this.workerCatalogs)
            if (!active.has(key) && !catalog.pending) {
              catalog.dispose();
              this.workerCatalogs.delete(key);
            }
        },
        { fireImmediately: true, equals: compareStructural },
      ),
      autorun(() => {
        const workspace = workspaceId();
        for (const agent of resourceData(
          scope.store.state().current,
        )?.agents.values() || []) {
          if (agent.isLead)
            this.control(
              agent,
              JSON.stringify([scope.services.owner.id, workspace, agent.id]),
              workspace,
            );
          const transfer = agent.accountTransferProgress;
          if (transfer?.kind === "blocked")
            appendAppLog({
              at: Date.now(),
              level: "error",
              title: "Account transfer blocked",
              dedupeKey: `account-transfer:${transfer.id}:${transfer.reason}`,
              reason: transfer.reason,
            });
        }
      }),
      reaction(selected, () => {
        this.clearReceipts();
        for (const key of this.fields.keys()) {
          this.set(key, "opened", false);
          this.set(key, "role", null);
        }
      }),
      autorun(() => {
        for (const [key, state] of this.fields) {
          const change = state.changeStatus;
          const agent =
            change &&
            resourceData(scope.store.state().current)?.agents.get(
              change.agentId,
            );
          const next =
            change && agent
              ? observeSettingsChange(change, agent, this.agent(agent.id) || {})
              : change;
          if (change !== next) {
            this.set(key, "changeStatus", next);
            if (next?.phase === "failed")
              appendAppLog({
                at: Date.now(),
                level: "error",
                title: "Execution settings change failed",
                reason: next.error || "The settings change failed.",
              });
          }
        }
      }),
    );
  }

  cell<K extends keyof Fields>(
    scope: string,
    key: K,
    initial: Fields[K] = defaults[key],
  ): [Fields[K], (value: Fields[K] | ((old: Fields[K]) => Fields[K])) => void] {
    const state = this.fields.get(scope);
    return [
      state ? state[key] : initial,
      (value) => this.set(scope, key, value),
    ];
  }
  set<K extends keyof Fields>(
    scope: string,
    key: K,
    value: Fields[K] | ((old: Fields[K]) => Fields[K]),
  ) {
    const state = this.fields.get(scope) ?? defaults;
    this.fields.set(scope, {
      ...state,
      [key]: typeof value === "function" ? value(state[key]) : value,
    });
  }

  begin(scope: string, role: SetupRole, read: () => string) {
    const receipt: Receipt = {
      value: { id: "" },
      baseline: read(),
      read,
    };
    this.set(scope, "confirmed", (old) => ({ ...old, [role]: receipt }));
    return (value: Agent) => {
      if (
        this.fields.get(scope)?.confirmed[role] !== receipt ||
        receipt.baseline !== read()
      )
        return false;
      this.set(scope, "confirmed", (old) => ({
        ...old,
        [role]: { ...receipt, value },
      }));
      return true;
    };
  }
  openSettings(id: string) {
    const scope = JSON.stringify([id, "model"]);
    this.set(
      scope,
      "role",
      this.scope &&
        resourceData(this.scope.store.state().current)?.agents.get(id)?.isLead
        ? "orchestrator"
        : "worker",
    );
    this.set(scope, "opened", true);
  }
  clearReceipts() {
    for (const key of this.fields.keys()) this.set(key, "confirmed", {});
  }
  agent(id: string) {
    return (
      this.scope &&
      resourceData(this.scope.store.agentDetail(agentId(id)).current)
    );
  }
  control(lead: Agent, scope: string, workspaceId: string) {
    if (!this.scope)
      throw new Error("A server scope is required for concurrency controls.");
    let control = this.concurrency.get(scope);
    if (!control) {
      control = new ConcurrencyStore(
        lead,
        this.scope.store,
        this.scope.services.api,
        this.scope.services.storage,
        scope,
        workspaceId,
      );
      this.concurrency.set(scope, control);
    }
    return control;
  }
  dispose() {
    this.stops.splice(0).forEach((stop) => stop());
    this.fields.clear();
    this.concurrency.clear();
    for (const catalog of this.workerCatalogs.values()) catalog.dispose();
    this.workerCatalogs.clear();
  }
}

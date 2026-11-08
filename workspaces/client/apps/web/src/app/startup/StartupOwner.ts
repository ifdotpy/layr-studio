import { autorun, makeAutoObservable, reaction, runInAction } from "mobx";
import { errorText } from "../../api.ts";
import { resourceData } from "../../store/runtime.ts";
import type { UiPreferenceProjection } from "../../sync/entityContracts.ts";
import {
  ReadRecoveryBudget,
  retryableReadError,
} from "../../sync/readRetry.ts";
import { selectMessageConnection } from "../../hooks/messageConnection.ts";
import type { AppScope, Disposer } from "../contracts.ts";

export interface StartupInputs {
  canUpdateRenderer(): boolean;
  acceptUiPreferences(preferences: UiPreferenceProjection): void;
}

/** Authentication, projection recovery and renderer compatibility share the server lifetime. */
export class StartupOwner {
  readonly scope: AppScope;
  readonly inputs: StartupInputs;
  workspaceId = "";
  startupError = "";
  projectionError = "";
  transportError = "";
  schemaMismatch = false;
  schemaMismatchNeedsRebuild = false;
  schemaUpdateError = "";
  rendererBusy = false;
  generation = 0;
  startupExhausted = false;
  credentialRecovery = new ReadRecoveryBudget();
  databaseRecovery = new ReadRecoveryBudget();
  initialized = false;
  running = false;
  credentialRetry: ReturnType<typeof setTimeout> | undefined;
  databaseRetry: ReturnType<typeof setTimeout> | undefined;
  stops: Disposer[] = [];

  constructor(scope: AppScope, inputs: StartupInputs) {
    this.scope = scope;
    this.inputs = inputs;
    makeAutoObservable(
      this,
      {
        scope: false,
        inputs: false,
        credentialRecovery: false,
        databaseRecovery: false,
        credentialRetry: false,
        databaseRetry: false,
        stops: false,
        generation: false,
        startupExhausted: false,
        initialized: false,
        running: false,
      },
      { autoBind: true },
    );
  }
  get error() {
    const error =
      this.startupError || this.projectionError || this.transportError;
    const state = this.scope.store.state().current;
    return error || (state.status === "error" ? errorText(state.error) : "");
  }
  get creationScope() {
    return (
      resourceData(this.scope.store.state().current)?.workspaces.values().next()
        .value?.stateDir || ""
    );
  }
  get isReady() {
    return this.scope.services.sync.projectionMetadata.ready;
  }
  get isSchemaGateVisible() {
    return this.schemaMismatch;
  }
  get updateRequiredTitle() {
    return this.schemaMismatchNeedsRebuild
      ? "Rebuild Studio"
      : "Studio update required";
  }
  get isRendererUpdateSafe() {
    return !this.rendererBusy && this.inputs.canUpdateRenderer();
  }
  showSchemaMismatch() {
    if (this.schemaMismatch) return;
    this.schemaMismatchNeedsRebuild =
      this.scope.services.api.schemaUpdateFailedAfterReload();
    this.schemaMismatch = true;
  }
  async retryUpdate() {
    this.schemaUpdateError = "";
    try {
      await this.scope.services.api.updateRendererAndReload();
    } catch (error) {
      if (this.running)
        runInAction(() => {
          this.schemaUpdateError =
            errorText(error) || "Studio update failed. Try again.";
        });
    }
  }
  async refresh(credentialsOnly = true, resetRecovery = true): Promise<void> {
    if (resetRecovery) {
      this.startupExhausted = false;
      this.credentialRecovery = new ReadRecoveryBudget();
      if (!this.initialized) {
        await this.restart();
        return;
      }
    }
    if (!this.running || this.credentialRecovery.exhaustedError) return;
    clearTimeout(this.credentialRetry);
    const request = ++this.generation;
    try {
      await this.scope.services.api.refreshSession();
      if (!this.running || request !== this.generation) return;
      if (!credentialsOnly) await this.scope.services.sync.refreshState();
      if (!this.running || request !== this.generation) return;
      this.startupExhausted = false;
      runInAction(() => {
        this.startupError = "";
      });
    } catch (error) {
      if (!this.running || request !== this.generation) return;
      const delay = this.reportStartupFailure(error, this.credentialRecovery);
      if (delay !== undefined && !document.hidden && navigator.onLine !== false)
        this.credentialRetry = setTimeout(() => {
          if (
            request === this.generation &&
            !document.hidden &&
            navigator.onLine !== false
          )
            void this.refresh(credentialsOnly, false);
        }, delay);
    }
  }
  reportStartupFailure(error: unknown, budget: ReadRecoveryBudget) {
    let reported = error;
    let delay: number | undefined;
    if (retryableReadError(error)) {
      try {
        delay = budget.nextDelay(error);
      } catch (exhausted) {
        reported = exhausted;
        this.startupExhausted = true;
      }
    }
    this.startupError = errorText(reported);
    return delay;
  }
  async initialize() {
    if (!this.running || this.databaseRecovery.exhaustedError) return;
    const request = this.generation;
    try {
      const storage = await this.scope.services.sync.syncDatabase();
      if (!this.running || request !== this.generation) return;
      this.initialized = true;
      runInAction(() => {
        this.workspaceId = storage.workspaceId;
      });
      await this.refresh(true, false);
    } catch (error) {
      if (!this.running || request !== this.generation) return;
      const delay = this.reportStartupFailure(error, this.databaseRecovery);
      if (delay !== undefined)
        this.databaseRetry = setTimeout(this.initialize, delay);
    }
  }
  async restart() {
    clearTimeout(this.databaseRetry);
    clearTimeout(this.credentialRetry);
    this.generation++;
    this.databaseRecovery = new ReadRecoveryBudget();
    this.credentialRecovery = new ReadRecoveryBudget();
    this.startupExhausted = false;
    if (this.initialized) await this.refresh(true, false);
    else await this.initialize();
  }
  start(): Disposer {
    this.running = true;
    const { api, resources, activity } = this.scope.services;
    this.rendererBusy = activity.serverActivityActive();
    let interrupted = false;
    this.stops = [
      api.onApiSchemaMismatch(this.showSchemaMismatch),
      activity.subscribeServerActivity(() =>
        runInAction(() => {
          this.rendererBusy = activity.serverActivityActive();
        }),
      ),
      resources.watchResourceConnection((status, activity, detail) => {
        runInAction(() => {
          this.transportError =
            status === "degraded" ||
            status === "offline" ||
            activity === "offline"
              ? selectMessageConnection(status, activity, detail).message
              : "";
        });
        if (status === "degraded" || status === "offline") interrupted = true;
        if (status === "live" && interrupted) {
          interrupted = false;
          void this.restart();
        }
      }),
      autorun(() => {
        const current = this.scope.store.state().current;
        const ready = this.isReady;
        runInAction(() => {
          this.projectionError =
            current.status === "error" ||
            (current.status === "stale" && current.error)
              ? errorText(current.error)
              : "";
          if (ready && !this.startupExhausted) this.startupError = "";
        });
      }),
      autorun(() => {
        const data = resourceData(this.scope.store.state().current);
        if (!this.isReady || !data) return;
        this.inputs.acceptUiPreferences({
          user: data.uiPreferences.get("user")
            ? { fields: data.uiPreferences.get("user")!.fields }
            : undefined,
          server: data.uiPreferences.get("server")
            ? { fields: data.uiPreferences.get("server")!.fields }
            : undefined,
        });
      }),
    ];
    let update: AbortController | undefined;
    this.stops.push(
      reaction(
        () =>
          this.scope.screen.active &&
          this.schemaMismatch &&
          this.isRendererUpdateSafe,
        (safe) => {
          update?.abort();
          if (!safe) return;
          const controller = (update = new AbortController());
          void api
            .automaticallyUpdateRenderer(
              () =>
                this.isRendererUpdateSafe &&
                !Array.from(document.querySelectorAll('[role="dialog"]')).some(
                  (dialog) =>
                    !dialog.closest(
                      '[role="alertdialog"][aria-label="Studio update required"]',
                    ),
                ),
              controller.signal,
            )
            .catch((error: unknown) => {
              if (!controller.signal.aborted && this.running)
                runInAction(() => {
                  this.schemaUpdateError = errorText(error);
                });
            });
        },
        { fireImmediately: true },
      ),
      () => update?.abort(),
    );
    void this.initialize();
    return this.dispose;
  }
  dispose() {
    this.running = false;
    this.generation++;
    clearTimeout(this.credentialRetry);
    clearTimeout(this.databaseRetry);
    this.stops
      .splice(0)
      .reverse()
      .forEach((stop) => stop());
  }
}

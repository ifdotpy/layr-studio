import { serverViewId, serverParentOrigin } from "../../servers/environment.ts";
import { action, autorun, makeAutoObservable, reaction } from "mobx";
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
  rendererBusy(): boolean;
  acceptUiPreferences(preferences: UiPreferenceProjection): void;
}

/** Time for cached state to paint before showing connection progress, in milliseconds. */
const STARTUP_CONNECTING_DELAY_MS = 800;

/** Authentication, projection recovery and renderer compatibility share the server lifetime. */
export class StartupOwner {
  readonly scope: AppScope;
  readonly inputs: StartupInputs;
  get workspaceId() {
    return this.scope.services.sync.currentScope?.workspaceId || "";
  }
  startupError = "";
  projectionError = "";
  transportError = "";
  schemaMismatch = false;
  schemaMismatchNeedsRebuild = false;
  schemaUpdateError = "";
  get rendererBusy() {
    return (
      this.inputs.rendererBusy() || this.scope.services.api.posting.count > 0
    );
  }
  generation = 0;
  credentialGeneration = 0;
  startupExhausted = false;
  credentialRecovery = new ReadRecoveryBudget();
  databaseRecovery = new ReadRecoveryBudget();
  initialized = false;
  running = false;
  connecting = false;
  connectingTimer: ReturnType<typeof setTimeout> | undefined;
  credentialRetry: ReturnType<typeof setTimeout> | undefined;
  databaseRetry: ReturnType<typeof setTimeout> | undefined;
  stops: Disposer[] = [];

  constructor(scope: AppScope, inputs: StartupInputs) {
    this.scope = scope;
    this.inputs = inputs;
    this.stops.push(
      scope.services.api.setMutationReadiness(
        () =>
          this.isReady && this.scope.store.state().current.status === "live",
      ),
    );
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
        credentialGeneration: false,
        startupExhausted: false,
        initialized: false,
        running: false,
        connectingTimer: false,
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
  get mutationReady() {
    return this.scope.services.api.mutationReady();
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
  *retryUpdate() {
    this.schemaUpdateError = "";
    try {
      yield this.scope.services.api.updateRendererAndReload();
    } catch (error) {
      if (this.running)
        this.schemaUpdateError =
          errorText(error) || "Studio update failed. Try again.";
    }
  }
  *refreshCredentials(resetRecovery = true): Generator<unknown, void, unknown> {
    if (resetRecovery) {
      this.startupExhausted = false;
      this.credentialRecovery = new ReadRecoveryBudget();
      if (!this.initialized) {
        yield this.restart();
        return;
      }
    }
    if (!this.running || this.credentialRecovery.exhaustedError) return;
    clearTimeout(this.credentialRetry);
    const request = ++this.credentialGeneration;
    try {
      yield this.scope.services.api.refreshSession();
      if (!this.running || request !== this.credentialGeneration) return;
      this.startupExhausted = false;
      this.startupError = "";
    } catch (error) {
      if (!this.running || request !== this.credentialGeneration) return;
      const delay = this.reportStartupFailure(error, this.credentialRecovery);
      if (delay !== undefined && !document.hidden && navigator.onLine !== false)
        this.credentialRetry = setTimeout(() => {
          if (
            request === this.credentialGeneration &&
            !document.hidden &&
            navigator.onLine !== false
          )
            void this.refreshCredentials(false);
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
  *initialize() {
    if (!this.running || this.databaseRecovery.exhaustedError) return;
    const request = this.generation;
    try {
      yield this.scope.services.sync.syncDatabase();
      if (!this.running || request !== this.generation) return;
      this.initialized = true;
    } catch (error) {
      if (!this.running || request !== this.generation) return;
      const delay = this.reportStartupFailure(error, this.databaseRecovery);
      if (delay !== undefined)
        this.databaseRetry = setTimeout(this.initialize, delay);
    }
  }
  *restart(): Generator<unknown, void, unknown> {
    [this.databaseRetry, this.credentialRetry].forEach(clearTimeout);
    this.generation++;
    this.databaseRecovery = new ReadRecoveryBudget();
    this.credentialRecovery = new ReadRecoveryBudget();
    this.startupExhausted = false;
    const credentials = this.refreshCredentials(false);
    if (!this.initialized) yield this.initialize();
    yield credentials;
  }
  receiveTransportError(message: string) {
    this.transportError = message;
  }
  receiveProjection(message: string, ready: boolean) {
    this.projectionError = message;
    if (ready && !this.startupExhausted) this.startupError = "";
  }
  receiveUpdateError(error: unknown) {
    this.schemaUpdateError = errorText(error);
  }
  start(): Disposer {
    this.running = true;
    this.connectingTimer = setTimeout(
      action(() => {
        this.connecting = true;
      }),
      STARTUP_CONNECTING_DELAY_MS,
    );
    const { api, resources } = this.scope.services;
    let interrupted = false;
    this.stops.push(
      api.onApiSchemaMismatch(this.showSchemaMismatch),
      reaction(
        () => this.rendererBusy,
        (busy) => {
          if (serverViewId && window.parent !== window)
            window.parent.postMessage(
              { kind: "studio-server-activity", serverId: serverViewId, busy },
              serverParentOrigin,
            );
        },
        { fireImmediately: true },
      ),
      resources.watchResourceConnection((status, activity, detail) => {
        this.receiveTransportError(
          status === "degraded" ||
            status === "offline" ||
            activity === "offline"
            ? selectMessageConnection(status, activity, detail).message
            : "",
        );
        if (status === "degraded" || status === "offline") interrupted = true;
        if (status === "live" && interrupted) {
          interrupted = false;
          void this.restart();
        }
      }),
      autorun(() => {
        const current = this.scope.store.state().current;
        const ready = this.isReady;
        this.receiveProjection(
          current.status === "error" ||
            (current.status === "stale" && current.error)
            ? errorText(current.error)
            : "",
          ready,
        );
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
    );
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
                this.receiveUpdateError(error);
            });
        },
        { fireImmediately: true },
      ),
      () => update?.abort(),
    );
    void this.refreshCredentials(false);
    void this.initialize();
    return this.dispose;
  }
  dispose() {
    this.running = false;
    clearTimeout(this.connectingTimer);
    this.generation++;
    [this.credentialRetry, this.databaseRetry].forEach(clearTimeout);
    for (const stop of this.stops.splice(0).reverse()) stop();
  }
}

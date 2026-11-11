import { ClaudeFileBackupsStore } from "../../components/conversation/ClaudeFileBackups/ClaudeFileBackups.tsx";
import { FederationSettingsStore } from "../../components/settings/FederationSettings/FederationSettings.tsx";
import { makeAutoObservable, reaction } from "mobx";
import { whileObserved } from "../../observed.ts";
import type { ServerApi, GetResult } from "../../api.ts";
import { errorText } from "../../api.ts";
type Configuration = GetResult<"/api/orchestration/settings">;
import type { ServerAreas } from "../contracts.ts";
import { ClaudeSettingsStore } from "../../components/settings/ClaudeSettingsView/ClaudeSettingsView.tsx";

export class OrchestrationSettingsStore {
  configuration: Configuration | null = null;
  error = "";
  busy = false;
  private generation = 0;
  readonly api;
  private readonly stops;
  constructor(api: Pick<ServerApi, "get" | "post">) {
    this.api = api;
    makeAutoObservable(this, { api: false }, { autoBind: true, deep: false });
    this.stops = [
      whileObserved(this, "configuration", () => {
        void this.load();
        return this.cancel;
      }),
    ];
  }
  cancel() {
    this.generation++;
  }
  setConfiguration(value: Configuration) {
    this.configuration = value;
  }
  patch(settings: Partial<Configuration["settings"]>) {
    if (this.configuration)
      this.configuration = {
        ...this.configuration,
        settings: { ...this.configuration.settings, ...settings },
      };
  }
  *load() {
    const generation = ++this.generation;
    try {
      const value: Configuration = yield this.api.get(
        "/api/orchestration/settings",
      );
      if (generation !== this.generation) return;
      this.configuration = value;
      this.error = "";
    } catch (failure) {
      if (generation === this.generation) this.error = errorText(failure);
    }
  }
  *save() {
    if (!this.configuration || this.busy) return;
    this.busy = true;
    this.error = "";
    try {
      this.configuration = yield this.api.post(
        "/api/orchestration/settings",
        this.configuration.settings,
      );
    } catch (failure) {
      this.error = errorText(failure);
    } finally {
      this.busy = false;
    }
  }
  dispose() {
    this.cancel();
    this.stops.forEach((stop) => stop());
  }
}

/** Constructed by the authenticated server, never by a component render. */
export class SettingsModels {
  readonly orchestration;
  readonly federation;
  readonly claude;
  fileBackups: ClaudeFileBackupsStore | undefined;
  private readonly stopBackups;
  constructor(areas: ServerAreas) {
    const api = areas.scope.services.api;
    this.orchestration = new OrchestrationSettingsStore(api);
    this.federation = new FederationSettingsStore(api, areas.notices.notify);
    makeAutoObservable(this, {}, { autoBind: true, deep: false });
    this.stopBackups = reaction(
      () => areas.navigation.selectedId,
      (id) => {
        this.fileBackups?.dispose();
        this.fileBackups = id ? new ClaudeFileBackupsStore(api, id) : undefined;
      },
      { fireImmediately: true },
    );
    this.claude = new ClaudeSettingsStore(
      api,
      () => areas.navigation.selectedAgent,
    );
  }
  dispose() {
    this.stopBackups();
    this.fileBackups?.dispose();
    this.orchestration.dispose();
    this.federation.dispose();
    this.claude.dispose();
  }
}

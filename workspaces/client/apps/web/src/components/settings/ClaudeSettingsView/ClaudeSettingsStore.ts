import { whileObserved } from "../../../observed.ts";
import { makeAutoObservable, flowResult, reaction } from "mobx";
import {
  ApiError,
  errorText,
  type ServerApi,
  type PostBody,
  type PostResult,
} from "../../../api.ts";
import type { Agent } from "../../../types.ts";
import { now } from "../../../clock.ts";
import {
  clearSettingsRequest,
  settingsRequestId,
} from "../../../app/settings/settingsRequestId.ts";

export class ClaudeSettingsStore {
  state: ClaudeSessionState = { settings: {}, turns: [], tasks: [] };
  stateLoaded = false;
  stateLoading = true;
  commands: ClaudeCommandsResponse = [];
  values: ClaudeValues;
  savedValues: ClaudeValues;
  window = "";
  command = "";
  error = "";
  busyAction = false;
  savingField = "";
  savedField = "";
  savedAt = 0;
  private generation = 0;
  private windowSaveTimer: ReturnType<typeof setTimeout> | undefined;
  private readonly stops;
  readonly api;
  private stopLoad = () => {};
  readonly agent;
  constructor(
    api: ServerApi | null,
    agent: () =>
      | Pick<Agent, "id" | "permissionMode" | "accountKey">
      | undefined,
  ) {
    this.api = api;
    this.agent = agent;
    this.values = this.savedValues = valuesFrom({}, agent() ?? {});
    makeAutoObservable(
      this,
      { api: false, agent: false },
      { autoBind: true, deep: false },
    );
    this.stops = [
      whileObserved(this, "state", () => {
        void this.observe();
        return this.cancel;
      }),
    ];
  }
  get id() {
    return this.agent()?.id ?? "";
  }
  observe() {
    this.stopLoad = reaction(() => this.id, this.load, {
      fireImmediately: true,
    });
  }
  get permissionMode() {
    return this.stateLoaded && !this.savingField
      ? this.values.permissionMode
      : null;
  }
  get windowValue() {
    return this.values.autoCompactWindow?.toString() ?? "";
  }
  get savedLabel() {
    return this.savedAt && now() - this.savedAt < 2200 ? "Saved" : "";
  }
  setCommand(value: string) {
    this.command = value;
  }
  setError(value: string) {
    this.error = value;
  }
  cancel() {
    this.stopLoad();
    this.generation++;
    clearTimeout(this.windowSaveTimer);
  }
  call(body: ClaudeSessionRequest) {
    if (!this.api) throw new Error("Claude settings need a server owner.");
    return this.api.post("/api/claude/session", body, { timeoutMs: 15000 });
  }
  *mutate(
    body: ClaudeSessionMutation,
  ): Generator<
    Promise<ClaudeSessionResponse>,
    ClaudeSessionResponse,
    ClaudeSessionResponse
  > {
    if (!this.api) throw new Error("Claude settings need a server owner.");
    const storageKey = `claude-control:${this.agent()?.accountKey}:${this.id}`;
    const requestId = settingsRequestId(
      this.api,
      storageKey,
      JSON.stringify(body),
    );
    try {
      const result = yield this.call({ ...body, request_id: requestId });
      clearSettingsRequest(this.api, storageKey, requestId);
      return result;
    } catch (error) {
      if (
        error instanceof ApiError &&
        error.status === 400 &&
        error.outcome !== "outcome_unknown"
      )
        clearSettingsRequest(this.api, storageKey, requestId);
      throw error;
    }
  }
  *load() {
    const generation = ++this.generation;
    clearTimeout(this.windowSaveTimer);
    this.busyAction = false;
    this.savingField = "";
    if (!this.id) return;
    this.stateLoaded = false;
    this.stateLoading = true;
    this.error = "";
    try {
      const result: ClaudeSessionResponse = yield this.call({
        id: this.id,
        action: "state",
      });
      if (generation !== this.generation) return;
      if (!isClaudeSessionState(result))
        throw new Error("Claude session state response is invalid.");
      this.state = result;
      this.values = this.savedValues = valuesFrom(result, this.agent() ?? {});
      this.window = this.windowValue;
      this.stateLoaded = true;
    } catch (failure) {
      if (generation === this.generation) {
        const message = errorText(failure);
        this.error =
          message === "Sign in with claude auth login first"
            ? "Sign in to Claude to load these settings."
            : message;
      }
    } finally {
      if (generation === this.generation) this.stateLoading = false;
    }
  }
  *saveSetting(field: string, patch: Partial<ClaudeValues>) {
    if (this.busyAction || this.savingField) return;
    const previous = this.savedValues,
      generation = this.generation;
    const next = { ...this.values, ...patch };
    this.values = next;
    this.savingField = field;
    this.savedAt = 0;
    const saved: boolean = yield flowResult(
      this.run(() =>
        flowResult(
          this.mutate({
            id: this.id,
            action: "settings",
            scope: { scope: "session" },
            settings: next,
          }),
        ),
      ),
    );
    if (generation !== this.generation) return;
    if (saved) {
      this.savedValues = next;
      this.savedField = field;
      this.savedAt = Date.now();
    } else {
      this.values = previous;
      this.window = this.windowValue;
    }
    this.savingField = "";
  }
  sendCommand(command: string, reload = false) {
    return this.run(
      () =>
        flowResult(this.mutate({ id: this.id, action: "command", command })),
      reload,
    );
  }
  stopTask(task_id: string) {
    return this.run(
      () =>
        flowResult(this.mutate({ id: this.id, action: "stop_task", task_id })),
      true,
    );
  }
  *run(work: () => Promise<unknown>, reload = false) {
    const generation = this.generation;
    this.busyAction = true;
    this.error = "";
    try {
      yield work();
      if (reload && generation === this.generation)
        yield flowResult(this.load());
      return generation === this.generation;
    } catch (failure) {
      if (generation === this.generation) this.error = errorText(failure);
      return false;
    } finally {
      if (generation === this.generation) this.busyAction = false;
    }
  }
  *readCommands() {
    yield flowResult(this.run(() => flowResult(this.fetchCommands())));
  }
  *fetchCommands() {
    const generation = this.generation;
    const result: ClaudeSessionResponse = yield this.call({
      id: this.id,
      action: "commands",
    });
    if (!Array.isArray(result))
      throw new Error("Claude commands response is invalid.");
    if (generation === this.generation) this.commands = result;
  }
  editWindow(input: string) {
    this.window = input;
    clearTimeout(this.windowSaveTimer);
    this.windowSaveTimer = setTimeout(() => this.saveWindow(input), 500);
  }
  saveWindow(input = this.window) {
    clearTimeout(this.windowSaveTimer);
    if (input === this.windowValue) return;
    if (!input.trim()) {
      void this.saveSetting("Auto-compact limit", {
        autoCompactWindow: undefined,
      });
      return;
    }
    const limit = Number(input);
    if (!Number.isInteger(limit) || limit < 100000 || limit > 1000000) {
      this.window = this.windowValue;
      this.error =
        "Enter a limit from 100,000 to 1,000,000, or leave it blank.";
      return;
    }
    void this.saveSetting("Auto-compact limit", { autoCompactWindow: limit });
  }
  dispose() {
    this.cancel();
    this.stops.forEach((stop) => stop());
  }
}

// An unowned static preview does not start a request.

export type ClaudeValues = {
  permissionMode: NonNullable<
    NonNullable<ClaudeSessionState["settings"]>["permissionMode"]
  >;
  thinking: boolean;
  autoCompactWindow?: number | undefined;
  mcpAutoBackground: boolean;
};

export function permissionMode(
  value: string,
): ClaudeValues["permissionMode"] | null {
  const modes: ClaudeValues["permissionMode"][] = [
    "default",
    "acceptEdits",
    "auto",
    "plan",
    "bypassPermissions",
  ];
  return modes.find((mode) => mode === value) ?? null;
}

type ClaudeSessionRequest = PostBody<"/api/claude/session">;

type ClaudeSessionResponse = PostResult<"/api/claude/session">;

type ClaudeSessionState = Extract<ClaudeSessionResponse, { turns: unknown[] }>;

type ClaudeCommandsResponse = Extract<ClaudeSessionResponse, unknown[]>;

type ClaudeSessionMutation = ClaudeSessionRequest extends infer Request
  ? Request extends {
      action: "settings" | "command" | "stop_task";
    }
    ? Omit<Request, "request_id">
    : never
  : never;

function isClaudeSessionState(
  value: ClaudeSessionResponse,
): value is ClaudeSessionState {
  return "turns" in value && Array.isArray(value.turns);
}

const valuesFrom = (
  value: Pick<ClaudeSessionState, "settings">,
  agent: Pick<Agent, "permissionMode">,
): ClaudeValues => ({
  permissionMode:
    value.settings?.permissionMode ||
    (agent.permissionMode ?? "bypassPermissions"),
  thinking: value.settings?.thinking !== false,
  mcpAutoBackground: value.settings?.mcpAutoBackground !== false,
  ...(value.settings?.autoCompactWindow
    ? { autoCompactWindow: Number(value.settings.autoCompactWindow) }
    : {}),
});

export const unloadedClaudeSettings = new ClaudeSettingsStore(
  null,
  () => undefined,
);

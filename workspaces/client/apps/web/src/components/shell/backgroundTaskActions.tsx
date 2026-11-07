import { resourceData } from "../../store/runtime.ts";
import { ProcessActionStatus } from "../../generated/api";
import { useMemo } from "react";
import { makeAutoObservable, runInAction } from "mobx";
import {
  ApiError,
  errorText,
  type ApiPostPath,
  type PostInput,
  type PostBody,
  type PostResult,
} from "../../api";
import { displayError } from "../../errorPresentation";
import { sessionOutboxRequests } from "../../browserPersistence";
import { randomUUID } from "../../uuid";
import { BackgroundTaskAction, BackgroundTaskKind } from "../../generated/api";
import {
  agentId,
  taskId,
  type ProcessList,
} from "../../generated/observableResources";
import {
  useServerApi,
  useServerServices,
  useServerStore,
} from "../../servers/ServerServicesContext";
import { createArea } from "../area";
import {
  processIdentity,
  type DisplayProcessTask,
  type DisplayBackgroundTask,
} from "../backgroundTaskModel";
import { labels } from "./backgroundTaskPresentation";
import { responseStopIdentity } from "./backgroundTaskIdentity";

export type BackgroundTaskProps = {
  opened: boolean;
  close(): void;
  afterClose?: (() => void) | undefined;
  leadId?: string | undefined;
  openAgent(id: string): void;
  refresh(): Promise<void>;
  notify(message: string): void;
  initialFocus?: { id: string; leadId: string; requestId: string } | undefined;
};
type Stop = PostBody<"/api/processes/action">;
type StopResult = PostResult<"/api/processes/action">;
const rejected = (error: unknown) =>
  error instanceof ApiError && error.status >= 400 && error.status < 500;

export function useBackgroundTaskArea(props: BackgroundTaskProps) {
  const api = useServerApi(),
    store = useServerStore();
  const serverId = useServerServices()?.owner.id || "local";
  const area = useMemo(() => {
    const leadId = props.leadId || "",
      outbox = sessionOutboxRequests();
    const suffix = `${serverId}:${leadId}`;
    let saved: Stop | null = null;
    try {
      saved = JSON.parse(outbox.get("processControl", suffix) || "null");
    } catch {
      /* No saved identity. */
    }
    class Panel {
      readonly leadId = leadId;
      kind = "all";
      query = "";
      mobileDetail = false;
      includeWorkers = false;
      selection = null as { id: string; requested?: boolean } | null;
      pending = false;
      controlPending = false;
      pendingControl = saved;
      controlResult = null as StopResult | null;
      disposed = false;
      close = props.close;
      openAgent = props.openAgent;
      notify = props.notify;
      refresh = props.refresh;
      get current() {
        return store.processes(agentId(leadId)).current;
      }
      get listing(): ProcessList | undefined {
        const slot = this.current;
        return resourceData(slot);
      }
      get activities(): ProcessList["activities"] {
        return this.listing?.activities ?? [];
      }
      get processTasks() {
        return this.activities.flatMap((entry) =>
          entry.source === "process" ? [entry.value] : [],
        );
      }
      get processLoadError() {
        const slot = this.current;
        return "error" in slot && slot.error ? errorText(slot.error) : "";
      }
      owner(id: string) {
        const slot = store.state().current;
        return resourceData(slot)?.agents.get(id);
      }
      get filtered() {
        return this.activities.filter(
          ({ value }) =>
            (this.kind === "all" || value.display.kind === this.kind) &&
            (!this.query ||
              `${value.display.title} ${this.owner(value.agent)?.name} ${value.cwd ?? ""} ${labels[value.display.label]}`
                .toLowerCase()
                .includes(this.query.toLowerCase())),
        );
      }
      get selectedEntry() {
        return this.selection
          ? this.activities.find(
              (entry) => entry.value.id === this.selection!.id,
            )
          : this.filtered[0];
      }
      get explicitId() {
        return this.selection?.requested ? this.selection.id : undefined;
      }
      get canStop() {
        const entry = this.selectedEntry;
        return (
          this.current.status !== "loading" &&
          !this.processLoadError &&
          !this.pendingControl &&
          entry?.source === "process" &&
          entry.value.display.actions.includes(BackgroundTaskAction.stop) &&
          (this.includeWorkers || entry.value.agent === leadId)
        );
      }
      retainSelection(task: DisplayBackgroundTask | undefined) {
        if (task && !this.selection) this.selection = { id: task.id };
      }
      select(task: DisplayBackgroundTask) {
        this.selection = { id: task.id };
        this.mobileDetail = true;
      }
      focus(id: string) {
        this.selection = { id, requested: true };
        this.kind = "all";
        this.query = "";
        this.mobileDetail = true;
      }
      filter(kind: string, query: string) {
        this.kind = kind;
        this.query = query;
        this.selection = null;
        this.mobileDetail = false;
      }
      back() {
        this.mobileDetail = false;
      }
      open(task: DisplayBackgroundTask) {
        this.close();
        this.openAgent(task.agent);
      }
      readOutput(task: DisplayBackgroundTask) {
        return api.get("/api/task/output", {
          query: { id: task.id, agent: task.agent },
        });
      }
      async checkInputReceipt(task: DisplayBackgroundTask) {
        const owner = this.owner(task.agent);
        if (owner?.epoch == null) return;
        return api.checkMonitorInputReceipt(task.id, {
          agent: owner.id,
          accountKey: owner.accountKey || "default",
          epoch: owner.epoch,
        });
      }
      restartReceipt(id: string): PostBody<"/api/monitor/restart"> | null {
        try {
          return JSON.parse(
            outbox.get("monitorRestart", serverId + ":" + id) || "null",
          );
        } catch {
          return null;
        }
      }
      async restart(body: PostBody<"/api/monitor/restart">) {
        const key = `${serverId}:${body.id}`;
        outbox.set("monitorRestart", key, JSON.stringify(body));
        try {
          await api.post("/api/monitor/restart", body);
          outbox.remove("monitorRestart", key);
          this.notify(
            "Restart queued. The new command waits for the previous command to exit.",
          );
          void this.refresh().catch((error) => this.notify(errorText(error)));
          return { status: "applied" } as const;
        } catch (error) {
          if (rejected(error)) {
            outbox.remove("monitorRestart", key);
            return { status: "rejected", error: errorText(error) } as const;
          }
          return {
            status: "unknown",
            error:
              "The restart result is unknown. Check the same request before another restart.",
          } as const;
        }
      }
      async run(work: () => Promise<unknown>) {
        this.pending = true;
        try {
          const result = await work();
          if (
            result &&
            typeof result === "object" &&
            "error" in result &&
            result.error
          )
            throw new Error(displayError(result.error));
          void this.refresh().catch((error) => this.notify(errorText(error)));
          return true;
        } catch (error) {
          this.notify(errorText(error));
          return false;
        } finally {
          runInAction(() => {
            this.pending = false;
          });
        }
      }
      act<Path extends ApiPostPath>(path: Path, body: PostInput<Path>) {
        return this.run(() => api.post(path, body));
      }
      approve(id: string) {
        return this.run(() => api.command.answer(id, { decision: "accept" }));
      }
      async downloadLog(task: DisplayBackgroundTask, fileText?: string) {
        try {
          const detail = store.taskDetail(taskId(task.id)).current;
          const monitor = task.display.kind === BackgroundTaskKind.monitor;
          const file = monitor
            ? await api.apiDownload("/api/monitor/log", { id: task.id })
            : {
                blob: new Blob(
                  [fileText ?? resourceData(detail)?.tail ?? task.tail ?? ""],
                  { type: "text/plain" },
                ),
                name: `command-${task.id}.log`,
                truncated: task.outputTruncated,
              };
          if (file.truncated)
            this.notify(
              monitor
                ? "The download contains the retained part of the log."
                : "The download contains the retained output.",
            );
          const url = URL.createObjectURL(file.blob),
            link = document.createElement("a");
          link.href = url;
          link.download = file.name;
          link.click();
          setTimeout(() => URL.revokeObjectURL(url), 10000);
        } catch (error) {
          this.notify(errorText(error));
        }
      }
      finish(result: StopResult) {
        this.controlResult = result;
        if (result.status === ProcessActionStatus.uncertain)
          this.notify(
            "The stop result is unknown. Check its receipt before another stop.",
          );
        else {
          this.remember(null);
          const outcomes = (result.processes || [])
            .map((item) => item.status)
            .join(", ");
          this.notify(
            result.status === ProcessActionStatus.partial
              ? `Some processes did not stop: ${outcomes || "see each process result"}`
              : `Stop result: ${outcomes || result.response?.status || "unknown"}`,
          );
        }
      }
      remember(body: Stop | null) {
        this.pendingControl = body;
        if (body) outbox.set("processControl", suffix, JSON.stringify(body));
        else outbox.remove("processControl", suffix);
      }
      async control(check = false) {
        const body = this.pendingControl;
        if (!body) return;
        this.controlPending = true;
        const unknown = check
          ? "The stop result is still unknown. Check its receipt again later."
          : "The stop reply is unknown. Check its receipt before another stop.";
        try {
          const { requestId, agent, accountKey, epoch } = body;
          const receipt = check
            ? await api.get("/api/processes/action", {
                query: { requestId, agent, accountKey, epoch },
              })
            : {
                found: true as const,
                result: await api.post("/api/processes/action", body, {
                  timeoutMs: 30000,
                }),
              };
          if (this.disposed) return;
          if (
            !receipt.found ||
            (check && receipt.result.status === ProcessActionStatus.uncertain)
          ) {
            this.notify(unknown);
            return;
          }
          this.finish(receipt.result);
          await this.refresh();
        } catch (error) {
          if (this.disposed) return;
          if (!check && rejected(error)) {
            this.remember(null);
            this.notify(errorText(error));
          } else {
            if (!check) this.remember(body);
            this.notify(unknown);
          }
        } finally {
          if (!this.disposed)
            runInAction(() => {
              this.controlPending = false;
            });
        }
      }
      runProcessControl(
        control:
          | { action: "stop_selected"; process: DisplayProcessTask }
          | { action: "stop_owned" | "stop_response" },
      ) {
        if (
          !leadId ||
          this.controlPending ||
          this.current.status === "loading" ||
          this.processLoadError
        )
          return;
        if (this.pendingControl) {
          this.notify(
            "Check the earlier stop result before starting another stop.",
          );
          return;
        }
        const target = this.listing?.responseTarget,
          owner = this.owner(leadId);
        const identity =
          control.action === "stop_response"
            ? target
            : owner?.epoch == null
              ? null
              : {
                  agent: leadId,
                  accountKey: owner.accountKey || "default",
                  epoch: owner.epoch,
                };
        if (!identity) {
          if (control.action === "stop_response")
            this.notify(
              "Refresh the process list before stopping this response.",
            );
          return;
        }
        const scope = {
          requestId: randomUUID(),
          agent: identity.agent,
          accountKey: identity.accountKey,
          epoch: identity.epoch,
        };
        let body: Stop;
        switch (control.action) {
          case "stop_response":
            body = {
              ...scope,
              action: control.action,
              ...responseStopIdentity(target!),
            };
            break;
          case "stop_selected":
            body = {
              ...scope,
              action: control.action,
              process: processIdentity(control.process),
              ...(control.process.agent !== leadId && this.includeWorkers
                ? { includeDescendants: true }
                : {}),
            };
            break;
          case "stop_owned":
            body = {
              ...scope,
              action: control.action,
              processes: this.processTasks
                .filter(
                  (task) =>
                    task.display.actions.includes(BackgroundTaskAction.stop) &&
                    (task.agent === leadId ||
                      (this.includeWorkers &&
                        this.owner(task.agent)?.rootId === leadId)),
                )
                .map(processIdentity),
              includeDescendants: this.includeWorkers,
            };
            break;
        }
        this.remember(body);
        void this.control();
      }
      stopSelected() {
        const entry = this.selectedEntry;
        if (entry?.source === "process")
          this.runProcessControl({
            action: "stop_selected",
            process: entry.value,
          });
      }
      constructor() {
        makeAutoObservable(
          this,
          {
            close: false,
            openAgent: false,
            notify: false,
            refresh: false,
            disposed: false,
          },
          { autoBind: true },
        );
      }
    }
    return new Panel();
  }, [api, store, serverId, props.leadId]);
  area.close = props.close;
  area.openAgent = props.openAgent;
  area.notify = props.notify;
  area.refresh = props.refresh;
  return area;
}
export type BackgroundTaskActions = ReturnType<typeof useBackgroundTaskArea>;
export const [BackgroundTaskActionsProvider, useBackgroundTaskActions] =
  createArea<BackgroundTaskActions>("Background task actions");

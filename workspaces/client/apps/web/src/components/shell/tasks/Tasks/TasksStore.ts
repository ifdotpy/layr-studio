import { downloadLog } from "../support/downloadLog.ts";
import { AsyncFeedback } from "../../../../stores/AsyncFeedback.ts";
import type { ShellFlow } from "../../support/flow.ts";
import { makeAutoObservable, observableRef, flowResult } from "mobx";
import {
  agentId as workAgentId,
  type WorkSubscriptionData,
} from "../../../../generated/observableResources.ts";
import { resourceData } from "../../../../store/runtime.ts";
import { ApiError, errorText, type PostBody } from "../../../../api.ts";
import type { ServerServices } from "../../../../servers/serverServices.ts";
import {
  serverMutationResponseIsUncertain,
  createMutationJournal,
} from "../../../../servers/mutationIdentity.ts";
export type WorkBody = PostBody<"/api/work">;
export type Task = WorkSubscriptionData["items"][number];
export function createTasksStore(
  rootId: string,
  api: ServerServices["api"],
  storage: ServerServices["storage"],
  store: ServerServices["store"],
) {
  const work = store.work(workAgentId(rootId)),
    root = store.agentDetail(workAgentId(rootId));
  const state = {
    selected: undefined as Task | undefined,
    status: "",
    owner: "",
    editing: null as "create" | "update" | null,
    busy: false,
    error: "",
    notice: "",
    pendingDecision: null as WorkBody | null,
    download: new AsyncFeedback(),
    get ready() {
      return !!resourceData(work.current) && !!resourceData(root.current);
    },
    get failure() {
      return (
        [work.current, root.current]
          .map((slot) => ("error" in slot ? slot.error?.message : ""))
          .find(Boolean) || this.error
      );
    },
    get gate() {
      return resourceData(root.current)?.taskDefaultGate ?? null;
    },
    get emptyMessage() {
      return this.tasks?.length ? "No tasks match these filters." : "No tasks.";
    },
    get tasks(): readonly Task[] | undefined {
      return resourceData(work.current)?.items;
    },
    get visibleTasks(): Task[] | undefined {
      return this.tasks?.filter(
        (task) =>
          (!this.status || task.policy.displayStatus === this.status) &&
          (!this.owner || (task.owner || "unassigned") === this.owner),
      );
    },
    get owners(): string[] {
      return Array.from(
        new Set(
          this.tasks?.flatMap((task) => (task.owner ? [task.owner] : [])) || [],
        ),
      );
    },
    get task(): Task | undefined {
      return this.tasks?.find((task) => task === this.selected);
    },
    select(task: Task) {
      if (this.selected !== task) {
        this.pendingDecision = null;
        this.download.error = "";
      }
      this.selected = task;
      this.edit(null);
    },
    edit(mode: "create" | "update" | null) {
      this.editing = mode;
      this.error = "";
    },
    *decide(
      task: Task,
      action: "accept" | "reject",
      reason: string,
      overrideReason: string,
    ): ShellFlow<void, boolean | "rejected"> {
      if (
        !this.pendingDecision &&
        (!reason.trim() ||
          (action === "accept" &&
            task.policy.gateOverrideRequired &&
            !overrideReason.trim()))
      )
        return;
      const payload = (this.pendingDecision ??= {
        action,
        task_id: task.id,
        version: task.version,
        result: reason.trim(),
        ...(action === "accept" && task.policy.gateOverrideRequired
          ? { override_reason: overrideReason.trim() }
          : {}),
      });
      if ((yield flowResult(this.write(() => payload))) !== false)
        this.pendingDecision = null;
    },
    downloadLog(id: string) {
      return flowResult(
        this.download.run(
          () => api.apiDownload("/api/monitor/log", { id }),
          (file) => {
            downloadLog(file);
            if (file.truncated)
              this.download.error =
                "The download contains the retained part of the log.";
          },
        ),
      );
    },
    *write(
      body: () => WorkBody,
    ): ShellFlow<
      boolean | "rejected",
      Awaited<ReturnType<ReturnType<typeof createMutationJournal>>>
    > {
      if (this.busy || !this.ready) return false;
      this.busy = true;
      this.error = "";
      this.notice = "";
      let finish: (() => void) | undefined;
      try {
        const payload = { ...body(), agent: rootId };
        const identity = yield createMutationJournal("task-review", storage)(
          "/api/work",
          payload,
        );
        finish = identity!.finish;
        yield api.post("/api/work", { ...payload, id: identity!.requestId });
        identity!.finish();
        this.notice = "The task action completed.";
        if (payload.action === "create" || payload.action === "update")
          this.edit(null);
        return true;
      } catch (failure) {
        this.error = errorText(failure);
        if (
          failure instanceof ApiError &&
          failure.status >= 400 &&
          failure.status < 500 &&
          !serverMutationResponseIsUncertain(failure.status, failure.response)
        ) {
          finish?.();
          return "rejected" as const;
        }
        return false;
      } finally {
        this.busy = false;
      }
    },
  };
  return makeAutoObservable(state, { selected: observableRef });
}
export type TasksStore = ReturnType<typeof createTasksStore>;

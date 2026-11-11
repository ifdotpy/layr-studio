import { StopOutcome } from "./generated/api.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  autorun,
  reaction,
  flowResult,
  makeAutoObservable,
  observableRef,
} from "mobx";
import { errorText } from "./api.ts";
import type {
  SidebarBackend,
  SidebarServices,
} from "./components/sidebar/support/services.tsx";
import type {
  StateCollections,
  ProjectionMetadata,
} from "./generated/observableResources.ts";
import { ProjectDeleteIntent, projectReaderScope } from "./projectDelete.ts";

type Project = NonNullable<ReturnType<StateCollections["projects"]["get"]>>;
type Options = {
  state: StateCollections | undefined;
  metadata: ProjectionMetadata;
  workspaceId: string;
  services: SidebarServices;
  onSuspend: (scope: ReadonlySet<string>) => void;
  onResume: (scope: ReadonlySet<string>) => void;
  onFinish: (ids: string[]) => void;
  notify: (message: string) => void;
};

/** One server's durable deletion intention and temporary reader suppression. */
export class ProjectDeletion {
  options: Options;
  target: { project: Project; seq: number; workspaceId: string } | null = null;
  pending: { intent: ProjectDeleteIntent; backend: SidebarBackend } | null =
    null;
  error = "";
  prior:
    | { workspaceId: string; stateDir: string; readers: Set<string> }
    | undefined;
  confirmed = { workspaceId: "", ids: new Set<string>() };
  restored = "";
  constructor(options: Options) {
    this.options = options;
    makeAutoObservable(
      this,
      {
        options: false,
        target: observableRef,
        pending: observableRef,
        prior: false,
        confirmed: false,
        restored: false,
        reconcile: false,
      },
      { autoBind: true },
    );
  }
  start() {
    let active = true;
    const reconcile = autorun(() => this.reconcile());
    const submit = reaction(
      () => this.pending,
      () => {
        queueMicrotask(() => {
          if (active) void flowResult(this.submit());
        });
      },
      { fireImmediately: true },
    );
    return () => {
      active = false;
      reconcile();
      submit();
    };
  }
  get complete() {
    const { state, metadata, workspaceId } = this.options;
    return (
      !!state &&
      !!workspaceId &&
      metadata.ready &&
      metadata.complete &&
      metadata.workspaceId === workspaceId
    );
  }
  get present() {
    const data = this.options.state;
    return new Set([
      ...(data?.agents.keys() ?? []),
      ...(data?.rooms.keys() ?? []),
      ...(data?.chats.keys() ?? []),
    ]);
  }
  publishSuppression() {
    const { workspaceId } = this.options;
    const scope = new Set(
      this.confirmed.workspaceId === workspaceId ? this.confirmed.ids : [],
    );
    const intent = this.pending?.intent;
    if (
      intent?.request.expected_workspace_id === workspaceId &&
      (intent.phase === "submitting" || intent.phase === "unknown")
    )
      for (const id of intent.scope) scope.add(id);
    if (scope.size) this.options.onSuspend(scope);
    else this.options.onResume(new Set());
  }
  reconcile() {
    const { state, workspaceId, services } = this.options;
    if (workspaceId && this.restored !== workspaceId) {
      this.restored = workspaceId;
      const previous = this.pending;
      if (
        previous &&
        previous.intent.request.expected_workspace_id !== workspaceId
      ) {
        this.pending = null;
        this.target = null;
        this.options.onResume(new Set(previous.intent.scope));
      }
      try {
        const intent = ProjectDeleteIntent.restore(
          services.cache.storage,
          workspaceId,
        );
        if (intent) {
          this.pending = { intent, backend: services.cache };
          this.publishSuppression();
        }
      } catch (failure) {
        this.error = errorText(failure);
        this.options.notify(this.error);
      }
    }
    if (!state || !this.complete) return;
    const stateDir = state.workspaces.values().next().value?.stateDir ?? "";
    const readers =
      this.prior?.workspaceId === workspaceId &&
      this.prior.stateDir === stateDir
        ? this.prior.readers
        : new Set<string>();
    this.prior = { workspaceId, stateDir, readers };
    const present = this.present;
    const ids = [...readers].filter((id) => !present.has(id));
    for (const id of ids) readers.delete(id);
    for (const project of state.projects.values())
      for (const id of projectReaderScope(
        project,
        [...state.agents.values()],
        [...state.rooms.values()],
      ))
        readers.add(id);
    if (
      this.confirmed.workspaceId &&
      this.confirmed.workspaceId !== workspaceId
    ) {
      this.confirmed.ids.clear();
      this.confirmed.workspaceId = workspaceId;
      this.publishSuppression();
    }
    if (this.confirmed.workspaceId === workspaceId) {
      const retired = [...this.confirmed.ids].filter((id) => !present.has(id));
      for (const id of retired) this.confirmed.ids.delete(id);
      if (retired.length) this.publishSuppression();
    }
    if (ids.length) this.options.onFinish(ids);
  }
  open(idOrPath: string) {
    if (this.pending) return;
    const { state, metadata, workspaceId } = this.options;
    const project =
      state?.projects.get(idOrPath) ??
      [...(state?.projects.values() ?? [])].find(
        (row) => row.path === idOrPath,
      );
    const seq = project ? metadata.projectSequences[project.id] : undefined;
    if (
      !project ||
      !this.complete ||
      seq === undefined ||
      !Number.isSafeInteger(seq)
    ) {
      this.options.notify(
        "Wait for the project's authoritative streamed state before deleting it.",
      );
      return;
    }
    this.error = "";
    this.target = { project, seq, workspaceId };
  }
  confirm() {
    const target = this.target;
    if (!target || this.pending) return;
    const { state, metadata, workspaceId, services } = this.options;
    if (!state || !workspaceId || !target.project.id || !target.project.path)
      return;
    if (
      workspaceId !== target.workspaceId ||
      metadata.workspaceId !== target.workspaceId
    ) {
      this.error =
        "The workspace changed. Close this dialog and confirm deletion again from the current project.";
      return;
    }
    try {
      const backend = services.owner({
        kind: "project",
        path: target.project.path,
      });
      const scope = projectReaderScope(
        target.project,
        [...state.agents.values()],
        [...state.rooms.values()],
      );
      const intent = new ProjectDeleteIntent(
        {
          id: target.project.id,
          request_id: randomUUID(),
          expected_workspace_id: workspaceId,
          expected_project_seq: target.seq,
        },
        target.project.name || target.project.path,
        [...scope],
        backend.storage,
      );
      this.pending = { intent, backend };
      this.publishSuppression();
    } catch (failure) {
      this.error = errorText(failure);
    }
  }
  *submit(): Generator<Promise<boolean>, void, boolean> {
    const pending = this.pending;
    if (!pending || pending.intent.phase !== "submitting") return;
    const submitted = yield pending.intent.submit(pending.backend.post);
    if (
      !submitted ||
      this.pending !== pending ||
      this.options.workspaceId !==
        pending.intent.request.expected_workspace_id ||
      this.options.metadata.workspaceId !==
        pending.intent.request.expected_workspace_id
    )
      return;
    const { intent } = pending;
    if (intent.phase === "deleted" && intent.result) {
      if (intent.result.stopped === StopOutcome.unconfirmed)
        this.options.notify(
          "The project was deleted, but stopping its commands could not be confirmed.",
        );
      if (intent.error) this.options.notify(intent.error);
      const ids = [
        ...intent.result.deletedAgentIds,
        ...intent.result.deletedRoomIds,
      ];
      if (this.confirmed.workspaceId !== this.options.workspaceId) {
        this.confirmed.workspaceId = this.options.workspaceId;
        this.confirmed.ids.clear();
      }
      const present = this.present;
      for (const id of ids)
        if (!this.complete || present.has(id)) this.confirmed.ids.add(id);
      this.publishSuppression();
      this.options.onFinish(ids);
      this.pending = null;
      this.target = null;
    } else {
      if (intent.phase === "rejected") {
        if (this.confirmed.ids.size) this.publishSuppression();
        else this.options.onResume(new Set(intent.scope));
        this.options.notify(intent.error);
      }
      this.pending = { ...pending };
    }
  }
  cancel() {
    if (this.pending && this.pending.intent.phase !== "rejected") return;
    this.pending = null;
    this.target = null;
    this.error = "";
  }
  recover() {
    const pending = this.pending;
    if (
      !pending ||
      pending.intent.phase !== "unknown" ||
      pending.intent.request.expected_workspace_id !== this.options.workspaceId
    )
      return;
    pending.intent.retry();
    this.pending = { ...pending };
  }
  get dialog() {
    return {
      name:
        this.pending?.intent.name ??
        this.target?.project.name ??
        this.target?.project.path ??
        null,
      phase:
        this.pending?.intent.phase === "deleted"
          ? ("submitting" as const)
          : (this.pending?.intent.phase ?? ("confirming" as const)),
      error: this.pending?.intent.error || this.error,
      onConfirm: this.confirm,
      onCancel: this.cancel,
      onRecover: this.recover,
    };
  }
}

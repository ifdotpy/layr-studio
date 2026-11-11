import { flow, makeAutoObservable, observableRef } from "mobx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  ApiError,
  errorText,
  type ApiPostPath,
  type GetResult,
  type PostBody,
  type PostResult,
} from "../../api.ts";
import type { ServerAreas } from "../ServerAreasProvider.tsx";
import { needsAnswer } from "../../approvals.ts";
import { flowValue } from "../../flowValue.ts";
import { AgentSource, WorkspaceMode } from "../../generated/api.ts";
import { agentId } from "../../generated/observableResources.ts";
import { providerSupports } from "../../generated/providerKind.ts";
import { serverMutationResponseIsUncertain } from "../../servers/mutationIdentity.ts";
import { resourceData } from "../../store/runtime.ts";
import { modelProvider } from "../../components/accounts/support/accountPreference.ts";
import {
  emptyCatalog,
  loadingCatalog,
  workerCatalogKey,
} from "../../components/agents/support/WorkerModelPicker.tsx";
import {
  openFilePreview,
  type FilePreviewSelection,
  type PreviewTarget,
} from "../../components/conversation/FilePreview/FilePreview.tsx";
import type { useServerStore } from "../../components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  isModelEnabled,
  readModelChoices,
} from "../../components/settings/models/support/modelChoices.ts";
import {
  modelDisplayName,
  modelForValue,
  toModelOptions,
} from "../../components/settings/models/support/modelOption.ts";
import type { WorkerLaunch } from "../../components/shell/WorkerLaunchDialog/WorkerLaunchDialog.tsx";
import type {
  Context,
  Props,
} from "../../components/shell/Workspace/Workspace.tsx";
import type { WorkspaceFieldsSpec } from "../../components/shell/WorkspaceFields/WorkspaceFields.tsx";
import type { WorkspaceResources } from "./WorkspaceResources.ts";
import {
  isRuleEvent,
  isRuleKind,
  newRuleDraft,
  parseRuleDraft,
  ruleBodyFromDraft,
  type RuleAction,
  type RuleDraft,
  type RuleRecord,
} from "./WorkspaceRules.ts";
import type { FormDraft } from "./FormDraft.ts";

type Comment = {
  path: string;
  line: number;
  turnId?: string;
  text: string;
  id: string;
};
type CheckpointPreview = Omit<
  PostResult<"/api/checkpoint/preview">,
  "checkpoint"
> & { checkpoint: string; label: string };
export type Profile = GetResult<"/api/profiles">["profiles"][number];
const profileEfforts = [
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
  "ultra",
] as const;
export const profileFields: WorkspaceFieldsSpec = {
  name: { label: "Name", required: true },
  role: {
    label: "Role",
    data: ["implementer", "reviewer"],
    fallback: "implementer",
  },
};
export const profileDetailsFields: WorkspaceFieldsSpec = {
  effort: {
    label: "Reasoning effort",
    data: [...profileEfforts],
    fallback: "high",
  },
  instructions: { label: "Instructions", minRows: 7, autosize: true },
};

const positiveInteger = {
  min: 1,
  allowDecimal: false,
  allowNegative: false,
  required: true,
};

const workerRuleOptions = [
  { value: "interval", label: "Repeat at an interval" },
  { value: "once", label: "Once at a time" },
  { value: "file", label: "File changes" },
  { value: "event", label: "Runtime event" },
];
const leadRuleOptions = [
  ...workerRuleOptions,
  { value: "low_workers", label: "Too few active subagents" },
];
const ruleEventOptions = [
  { value: "", label: "Choose an event" },
  { value: "worker_completed", label: "Subagent completes a task" },
  "monitor_exit",
  "complaint",
  "work_review",
];

type Entry = Pick<Props, "opened" | "agent" | "initialSection">;
const diffHunkPattern = /^@@ -\d+(?:,\d+)? \+(\d+)/;
const diffPathPattern = / b\/(.*)$/;

/** Independent section interaction state shares the existing mutation lifecycle. */
export class WorkspacePanel<T> {
  busy = false;
  remove: T | null = null;
  comment: Comment | null = null;
  preview: CheckpointPreview | null = null;
  label = "";
  launch: WorkerLaunch | null = null;
  constructor() {
    makeAutoObservable(
      this,
      {
        comment: observableRef,
        preview: observableRef,
        remove: observableRef,
        launch: observableRef,
      },
      { autoBind: true },
    );
  }
  submit = flow(function* <P extends ApiPostPath>(
    this: WorkspacePanel<T>,
    c: Pick<Context, "run" | "reload" | "notify"> & {
      model: Pick<WorkspaceStore, "submittingPanels">;
    },
    path: P,
    body: PostBody<P>,
    done: (value: PostResult<P>) => void = () => {},
    form?: FormDraft,
  ) {
    this.busy = true;
    c.model.submittingPanels.add(this);
    const submitted = form?.serialized ?? null;
    try {
      const result = yield* flowValue(c.run(path, body));
      c.reload();
      done(result);
      if (form) yield* flowValue(form.clear(submitted));
    } catch (error) {
      c.notify(errorText(error));
      if (
        form &&
        error instanceof ApiError &&
        !serverMutationResponseIsUncertain(error.status, error.response)
      )
        yield* flowValue(form.clear(submitted));
    } finally {
      this.busy = false;
      c.model.submittingPanels.delete(this);
    }
  });
  saveComment(c: Context) {
    if (!c.selected || !this.comment) return;
    return this.submit(
      c,
      "/api/annotation",
      { agent: c.selected.id, ...this.comment },
      () => (this.comment = null),
    );
  }
  saveCheckpoint(c: Context) {
    if (!c.selected) return;
    return this.submit(
      c,
      "/api/checkpoint",
      {
        agent: c.selected.id,
        label: this.label.trim() || "Manual checkpoint",
      },
      () => (this.label = ""),
    );
  }
  previewCheckpoint(
    c: Context,
    checkpoint: GetResult<"/api/checkpoints">["checkpoints"][number],
  ) {
    if (!c.selected) return;
    return this.submit(
      c,
      "/api/checkpoint/preview",
      { agent: c.selected.id, checkpoint: checkpoint.id },
      (value) =>
        (this.preview = {
          ...value,
          checkpoint: checkpoint.id,
          label: checkpoint.label ?? "Checkpoint",
        }),
    );
  }
  restoreCheckpoint(c: Context) {
    const preview = this.preview;
    if (!c.selected || !preview) return;
    return this.submit(
      c,
      "/api/checkpoint/restore",
      {
        agent: c.selected.id,
        checkpoint: preview.checkpoint,
        expectedTree: preview.expectedTree,
      },
      () => (this.preview = null),
    );
  }
  saveProfile(c: Context, model: string) {
    const draft = c.model.profileDraft.value;
    if (!draft) return;
    return this.submit(
      c,
      "/api/profiles",
      {
        action: "save",
        id: typeof draft.id === "string" ? draft.id : undefined,
        isNew: draft.isNew === true,
        name: typeof draft.name === "string" ? draft.name : "",
        role: draft.role === "reviewer" ? "reviewer" : "implementer",
        model,
        effort:
          profileEfforts.find((effort) => effort === draft.effort) ?? null,
        instructions:
          typeof draft.instructions === "string" ? draft.instructions : "",
      },
      undefined,
      c.model.profileDraft,
    );
  }
  deleteProfile(this: WorkspacePanel<Profile>, c: Context) {
    const remove = this.remove;
    return this.submit(
      c,
      "/api/profiles",
      {
        id: remove?.id,
        action: "delete",
        role: remove?.role ?? "reviewer",
        instructions: remove?.instructions ?? "",
      },
      () => (this.remove = null),
    );
  }
  launchProfile(c: Context, body: PostBody<"/api/agents">) {
    return this.submit(c, "/api/agents", body, (worker) => {
      this.launch = null;
      c.notify(`Started ${worker.name}`);
    });
  }
  saveRule(c: Context) {
    if (!c.model.ruleDraft || !c.selected) return;
    return this.submit(
      c,
      "/api/rules",
      ruleBodyFromDraft(c.model.ruleDraft, c.selected.id),
      undefined,
      c.model.ruleForm,
    );
  }
  ruleAction(c: Context, rule: RuleRecord, action: RuleAction) {
    if (!c.selected) return;
    return this.submit(
      c,
      "/api/rules",
      { agent: c.selected.id, id: rule.id, action },
      () => {
        if (action === "delete") this.remove = null;
      },
    );
  }
}

/** Server-owned selection, derived panel data and workspace command actions. */
export class WorkspaceStore {
  readonly submittingPanels = new Set<{ busy: boolean }>();
  readonly areas;
  readonly resources;
  readonly server;
  entry: Entry;
  section: string;
  agentId: string;
  preview: FilePreviewSelection | null = null;
  previewFile(target: PreviewTarget | null) {
    this.preview = target
      ? openFilePreview(this.areas.scope.services.net, target)
      : null;
  }
  filePath = "";
  constructor(
    resources: WorkspaceResources,
    server: ReturnType<typeof useServerStore>,
    entry: Entry,
    areas: ServerAreas,
  ) {
    this.areas = areas;
    this.resources = resources;
    this.server = server;
    this.entry = entry;
    this.section = entry.initialSection || "messages";
    this.agentId = entry.agent?.id || "";
    makeAutoObservable(
      this,
      {
        areas: false,
        resources: false,
        server: false,
        entry: observableRef,
        preview: observableRef,
      },
      { autoBind: true },
    );
  }
  enter(next: Entry) {
    const previous = this.entry;
    if (
      previous.opened !== next.opened ||
      previous.agent?.id !== next.agent?.id ||
      previous.initialSection !== next.initialSection
    ) {
      this.entry = next;
      if (next.opened) {
        this.agentId = next.agent?.id || "";
        if (
          next.initialSection &&
          (!previous.opened || previous.initialSection !== next.initialSection)
        )
          this.section = next.initialSection;
      }
    }
  }
  get state() {
    return resourceData(this.server.state().current);
  }
  get stateDir() {
    return this.state?.workspaces.values().next().value?.stateDir ?? "";
  }
  get rootId() {
    return this.entry.agent?.rootId || this.entry.agent?.id;
  }
  get agents() {
    return [...(this.state?.agents.values() ?? [])].filter(
      (a) =>
        !!this.rootId && (a.id === this.rootId || a.rootId === this.rootId),
    );
  }
  get managedAgents() {
    return this.agents.filter((a) => a.source === AgentSource.managed);
  }
  get selected() {
    return this.managedAgents.find((a) => a.id === this.agentId);
  }
  get attention() {
    const members = new Set(this.agents.map((a) => a.id));
    return (
      [
        ...(resourceData(this.server.requests().current)?.requests.values() ??
          []),
      ].filter((r) => members.has(r.agent) && needsAnswer(r)).length +
      [...(this.state?.complaints.values() ?? [])].filter(
        (c) => c.leadId === this.rootId && c.needsUserResponse,
      ).length
    );
  }
  select(id: string) {
    this.agentId = id;
  }
  navigate(section: string, agent?: string) {
    this.section = section;
    if (agent) this.select(agent);
  }
  setFilePath(path: string) {
    this.filePath = path;
  }
  openSelected(props: Props) {
    if (this.selected) {
      props.onSelect(this.selected.id);
      props.onClose();
    }
  }
  get changeState() {
    return this.resources.reads["/api/changes"]({
      ...(this.selected ? { agent: this.selected.id } : {}),
      scope: "chat",
    });
  }
  get report() {
    const data = this.changeState.data;
    return data?.scope === "chat" ? data : null;
  }
  get files() {
    return this.report?.files ?? [];
  }
  get annotations() {
    return [...(this.state?.annotations.values() ?? [])].filter(
      (e) => e.agent === this.selected?.id,
    );
  }
  get rows() {
    let line = 0,
      current = "",
      inHunk = false;
    return String(this.report?.diff || "")
      .split("\n")
      .map((text) => {
        if (text.startsWith("diff --git ")) {
          current = "";
          inHunk = false;
        }
        if (!inHunk && text.startsWith("+++ b/")) current = text.slice(6);
        const hunk = text.match(diffHunkPattern);
        if (hunk) {
          line = Number(hunk[1]) - 1;
          inHunk = true;
        }
        const newLine =
          inHunk && !hunk && (text.startsWith("+") || text.startsWith(" "));
        if (newLine) line++;
        const headerPath = text.startsWith("diff --git ")
          ? text.match(diffPathPattern)?.[1] || text.slice(11)
          : undefined;
        return {
          text,
          line,
          path: current,
          headerPath,
          commentable: !!current && newLine,
        };
      });
  }

  get checkpointState() {
    return this.resources.reads["/api/checkpoints"](
      this.selected ? { agent: this.selected.id } : {},
    );
  }
  get canRewind() {
    const detail = this.selected
      ? resourceData(this.server.agentDetail(agentId(this.selected.id)).current)
      : undefined;
    return (
      providerSupports(this.selected?.provider, "rewind") &&
      (detail?.worktree || detail?.workspaceMode === WorkspaceMode.worktree) &&
      detail?.workspaceMode !== WorkspaceMode.shared
    );
  }

  newProfile() {
    this.profileDraft.update({
      id: randomUUID(),
      isNew: true,
      name: "",
      role: "implementer",
      model: "",
      effort: "high",
      instructions: "",
    });
  }
  beginProfileLaunch(ui: WorkspacePanel<Profile>, profile: Profile) {
    ui.launch = {
      id: randomUUID(),
      profile_id: profile.id,
      provider: modelProvider(profile.model) ?? this.lead?.provider ?? "",
      reviewer: profile.role === "reviewer",
      name: profile.name,
    };
  }
  get profileState() {
    return this.resources.reads["/api/profiles"](undefined);
  }
  get profileDraft() {
    return this.resources.form(`studio-profile-draft:${this.stateDir}`);
  }

  get ruleState() {
    return this.resources.reads["/api/rules"](
      this.selected ? { agent: this.selected.id } : {},
    );
  }
  get rules() {
    return (this.ruleState.data?.rules ?? [])
      .filter((rule) => rule.agent === this.selected?.id)
      .map((rule) => {
        const paused = rule.status === "paused" || rule.enabled === false;
        const toggleAction: RuleAction = paused ? "resume" : "pause";
        return {
          rule,
          paused,
          toggleAction,
          toggleLabel: paused ? "Resume" : "Pause",
        };
      });
  }
  newRule() {
    this.setRuleDraft(newRuleDraft(randomUUID()));
  }
  get ruleForm() {
    return this.resources.form(
      `studio-rule-draft:${JSON.stringify([this.stateDir, this.selected?.id])}`,
    );
  }
  get ruleDraft() {
    return parseRuleDraft(this.ruleForm.value);
  }
  setRuleDraft(value: RuleDraft | null) {
    this.ruleForm.update(value ? { ...value } : null);
  }
  get ruleFields(): WorkspaceFieldsSpec {
    const kind = this.ruleDraft?.kind;
    return {
      name: { label: "Name", required: true },
      kind: {
        label: "Trigger",
        data: this.selected?.isLead ? leadRuleOptions : workerRuleOptions,
      },
      minimumWorkers: kind === "low_workers" && {
        label: "Minimum active subagents",
        ...positiveInteger,
        max: 255,
      },
      durationMinutes: kind === "low_workers" && {
        label: "Duration (minutes)",
        ...positiveInteger,
        max: 525600,
      },
      intervalSeconds: kind === "interval" && {
        label: "Interval (seconds)",
        required: true,
        min: 10,
      },
      at: kind === "once" && {
        label: "Run at (local time)",
        required: true,
        type: "datetime-local",
      },
      path: kind === "file" && {
        label: "File path",
        required: true,
        description: "Relative to this agent's workspace.",
      },
      event: kind === "event" && {
        label: "Runtime event",
        required: true,
        data: ruleEventOptions,
      },
      command: kind !== "low_workers" && {
        label: "Script check (optional)",
        minRows: 3,
        description:
          'Run under the agent permissions. Exit 0 wakes the agent unless the output contains {"wakeAgent":false}.',
      },
      text: {
        label:
          kind === "low_workers"
            ? "Message to the main agent"
            : "Message to the agent",
        required: true,
        minRows: 3,
      },
    };
  }
  changeRule(key: string, value: string | number) {
    const draft = this.ruleDraft;
    if (!draft) return;
    if (key === "kind") {
      if (typeof value !== "string" || !isRuleKind(value)) return;
      this.setRuleDraft({
        ...draft,
        kind: value,
        ...(value === "low_workers"
          ? {
              name: draft.name.trim() ? draft.name : "Too few active subagents",
              text: draft.text.trim()
                ? draft.text
                : "Fewer subagents than the minimum were active.",
            }
          : {}),
      });
    } else if (
      key !== "event" ||
      (typeof value === "string" && isRuleEvent(value))
    ) {
      this.ruleForm.change(
        key,
        key === "intervalSeconds" ? Number(value) : value,
      );
    }
  }
  get lead() {
    return this.state?.agents.get(
      this.selected?.rootId || this.selected?.id || "",
    );
  }
  get catalog() {
    if (!this.profileDraft.value) return emptyCatalog;
    return (
      this.areas.agentsUi.workerCatalogs.get(
        workerCatalogKey(this.lead?.accountKey || "default", true),
      ) ?? loadingCatalog
    );
  }
  get planState() {
    return {
      data: this.state?.plans.get(this.selected?.id ?? "") ?? null,
      error: "",
      loading: !this.areas.scope.services.sync.projectionMetadata.complete,
    };
  }
  get profileModel() {
    const draft = this.profileDraft.value,
      catalog = this.catalog,
      lead = this.lead;
    const allowed = readModelChoices(this.areas.scope.services.storage);
    const profileModels = toModelOptions(catalog.models);
    const eligible = catalog.models.filter(
      (model) => !model.hidden && isModelEnabled(model, allowed),
    );
    const fallback =
      (eligible.find((model) => model.isDefault) || eligible[0])?.model || "";
    const draftModel =
      (typeof draft?.model === "string" ? draft.model : "") ||
      (draft?.isNew ? fallback : "");
    const validModel =
      !!draftModel &&
      (!draft?.isNew || eligible.some((model) => model.model === draftModel));
    if (draftModel && !modelForValue(profileModels, draftModel))
      profileModels.unshift({
        value: draftModel,
        label: modelDisplayName(draftModel),
        provider: modelProvider(draftModel) ?? lead?.provider ?? "",
        badges: ["current"],
      });
    return { profileModels, draftModel, validModel };
  }
}

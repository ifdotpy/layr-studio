import {
  createServerServices,
  type ServerServices,
} from "../../servers/serverServices.ts";
import {
  projectLocationOn,
  type LocationProjectView,
  type ProjectServerChoice,
} from "../../servers/projectLocations.ts";
import { ProjectServerRead } from "../../components/projects/ProjectServerCards/ProjectServerCards.tsx";
import { NewChatModelStore } from "../../components/settings/NewChatModelSettings/NewChatModelSettings.tsx";
import { flowValue } from "../../flowValue.ts";
import { makeAutoObservable, reaction, flow, compareShallow } from "mobx";
import { MarkUnreadEligibility } from "../../generated/api.ts";
import {
  errorText,
  untilAnswered,
  type PostBody,
  type PostResult,
} from "../../api.ts";
import type { Json } from "../../types.ts";
import {
  projectAllowsWorktree,
  type NewChatSettings,
} from "./newChatSettings.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import type { AgentModel, ServerAreas, SelectionIntent } from "../contracts.ts";
type LeadCreateRequest = PostBody<"/api/leads"> &
  Required<Pick<PostBody<"/api/leads">, "previous" | "reuse_empty">> & {
    id: string;
  };
export function isLeadCreateRequest(value: Json): value is LeadCreateRequest {
  return (
    typeof value.id === "string" &&
    !!value.id &&
    (value.previous === null || typeof value.previous === "string") &&
    typeof value.reuse_empty === "boolean" &&
    (value.cwd === undefined || typeof value.cwd === "string") &&
    (value.project_folder === undefined ||
      typeof value.project_folder === "string") &&
    (value.account_key === undefined || typeof value.account_key === "string")
  );
}
export type SharedCreation = {
  body: Extract<
    PostBody<"/api/peer-teams">,
    { action: "radio"; radio_action: "create" }
  >;
  roomId?: string;
  rejected?: boolean;
};
export const sharedCreationKey = (scope: string) =>
  `studio-radio-create:${scope}`;
export type ProjectBinding = {
  projectId: string;
  homeServerId: string;
};
/** Durable creation identity is settled before any scope-sensitive UI action. */
export class CreationOwner {
  readonly areas;
  private receipts: {
    scope: string;
    agents: AgentModel[];
  };
  private pending: Json[] = [];
  private disposed = false;
  private generation = 0;
  private stop = () => {};
  isCreating = false;
  sharedAttempt: SharedCreation | null = null;
  sharedCreate: {
    initialPath?: string | undefined;
    intent: SelectionIntent;
  } | null = null;
  constructor(areas: ServerAreas) {
    this.areas = areas;
    this.receipts = areas.scope.services.api.saved("codex-confirmed-chats", {
      scope: "",
      agents: [],
    });
    makeAutoObservable<this, "stop">(
      this,
      { areas: false, stop: false, ensureConversation: false },
      { autoBind: true, deep: false },
    );
  }
  get creationKey() {
    return `codex-pending-creation:${this.areas.startup.creationScope}`;
  }
  get pendingCreationRequests() {
    return this.pending.filter(isLeadCreateRequest);
  }
  get hasPendingCreation() {
    return this.pendingCreationRequests.length > 0;
  }
  get suggestedProject() {
    const lead =
      this.areas.navigation.selectedLead || this.areas.navigation.selectedAgent;
    return lead?.projectPath
      ? {
          path: lead.projectPath,
          folder: lead.projectFolder || undefined,
          leadId: lead.id,
        }
      : undefined;
  }
  get sharedCreationInitialPath() {
    return this.sharedCreate?.initialPath ?? this.suggestedProject?.path;
  }
  start() {
    this.disposed = false;
    const { api, storageEventKey } = this.areas.scope.services;
    const stopScope = reaction(
      () => ({
        scope: this.areas.startup.creationScope,
        agents: this.unreplicatedCreatedAgents,
        replicatedRoom:
          this.sharedCreate &&
          this.sharedAttempt?.roomId &&
          this.areas.state?.rooms.has(this.sharedAttempt.roomId)
            ? this.sharedAttempt.roomId
            : null,
      }),
      ({ scope, replicatedRoom }, previous) => {
        if (scope !== previous?.scope) {
          this.generation++;
          this.isCreating = false;
          this.reloadPending();
          this.closeShared();
          if (api.saved(`studio-radio-create:${scope}`, null))
            this.openShared();
        }
        this.acknowledgeReplication();
        if (replicatedRoom) {
          this.rememberShared(null);
          this.sharedCreated(replicatedRoom);
        }
      },
      { fireImmediately: true },
    );
    const receive = (event: StorageEvent) => {
      const key = storageEventKey(event);
      if (
        key === this.creationKey ||
        key?.startsWith(`${this.creationKey}:request:`)
      )
        this.reloadPending();
    };
    window.addEventListener("storage", receive);
    this.stop = () => {
      stopScope();
      window.removeEventListener("storage", receive);
    };
    return this.dispose;
  }
  dispose() {
    this.generation++;
    this.disposed = true;
    this.stop();
    this.closeShared();
  }
  private isCurrent(intent: SelectionIntent, generation = this.generation) {
    return (
      !this.disposed &&
      generation === this.generation &&
      this.areas.startup.creationScope === intent.scope
    );
  }
  acceptCreated(
    receipt: PostResult<"/api/leads">,
    intent: SelectionIntent,
    generation = this.generation,
  ) {
    if (!this.isCurrent(intent, generation)) return;
    this.receipts = {
      scope: intent.scope,
      agents: [
        ...(this.receipts.scope === intent.scope
          ? this.receipts.agents
          : []
        ).filter((row) => row.id !== receipt.id),
        {
          ...receipt,
          inFlight: receipt.inFlight ?? false,
          markRead: null,
          markUnread: MarkUnreadEligibility.no_completed_result,
          operationalStatus: { reason: "ready" },
          projectServerId: receipt.projectServerId ?? null,
        },
      ],
    };
    this.persist();
    this.areas.navigation.acceptCreated(receipt, intent, intent.scope);
  }
  ensureConversation = flow(function* (
    this: CreationOwner,
    settings?: NewChatSettings,
  ) {
    return (
      this.areas.navigation.selectedId ||
      ((yield* flowValue(
        this.newChat(undefined, undefined, undefined, undefined, settings),
      )) ??
        undefined)
    );
  });
  retry(request: LeadCreateRequest) {
    return this.newChat(undefined, undefined, request.id);
  }
  dismiss(request: LeadCreateRequest) {
    const { chatCreations } = this.areas.scope.services;
    try {
      chatCreations.dismissChatCreation(this.creationKey, request.id);
      this.reloadPending();
    } catch (failure) {
      this.areas.notices.notify(
        "Cannot dismiss the saved chat request: " + errorText(failure),
      );
    }
  }
  rememberShared(
    value: SharedCreation | null,
    scope = this.areas.startup.creationScope,
  ) {
    const api = this.areas.scope.services.api,
      key = sharedCreationKey(scope),
      previous = api.saved<SharedCreation | null>(key, null);
    if (
      value &&
      (value.roomId || value.rejected) &&
      previous?.body.request_id !== value.body.request_id
    )
      return;
    api.save(key, value);
    if (scope === this.areas.startup.creationScope) this.sharedAttempt = value;
  }
  openShared(initialPath?: string) {
    this.sharedAttempt = this.areas.scope.services.api.saved(
      sharedCreationKey(this.areas.startup.creationScope),
      null,
    );
    this.sharedCreate = {
      initialPath,
      intent: this.areas.navigation.captureIntent(),
    };
  }
  closeShared() {
    this.sharedCreate = null;
  }
  sharedCreated(id: string) {
    const intent = this.sharedCreate?.intent,
      current = this.areas.navigation.captureIntent();
    this.closeShared();
    if (!intent || !this.isCurrent(intent) || intent.intent !== current.intent)
      return;
    this.areas.open(id);
  }
  reloadPending() {
    const { chatCreations } = this.areas.scope.services;
    try {
      this.pending = chatCreations.pendingChatCreations(this.creationKey);
    } catch (failure) {
      this.areas.notices.notify(
        "Cannot read the saved chat request: " + errorText(failure),
      );
    }
  }
  newChat = flow(function* (
    this: CreationOwner,
    cwd?: string,
    folder?: string,
    retryId?: string,
    binding?: ProjectBinding,
    settings?: NewChatSettings,
  ) {
    if (this.isCreating || this.disposed) return null;
    if (!retryId && !cwd) {
      cwd = this.suggestedProject?.path;
      folder = this.suggestedProject?.folder;
    }
    if (!retryId && !settings)
      return { kind: "needs-project", cwd, folder, binding };
    const body = {
      cwd,
      project_folder: folder,
      project_id: binding?.projectId,
      project_server_id: binding?.homeServerId,
      ...settings,
    };
    const { api, chatCreations } = this.areas.scope.services;
    const key = this.creationKey;
    let savedRequest: LeadCreateRequest | null;
    try {
      const pending = chatCreations.pendingChatCreations(key);
      this.pending = pending;
      savedRequest =
        pending.find((request): request is LeadCreateRequest => {
          if (!isLeadCreateRequest(request)) return false;
          if (retryId) return request.id === retryId;
          return Object.entries(body).every(([field, value]) => {
            const actual =
              field === "cwd"
                ? request.cwd ||
                  this.lookup(request.previous || "")?.projectPath
                : (Reflect.get(request, field) ?? undefined);
            return JSON.stringify(actual) === JSON.stringify(value);
          });
        }) ?? null;
      if (retryId && !savedRequest)
        throw new Error("The saved chat request is no longer available.");
    } catch (failure) {
      this.areas.notices.notify(
        "Cannot read the saved chat request: " + errorText(failure),
      );
      return null;
    }
    const intent = this.areas.navigation.captureIntent(),
      generation = this.generation;
    const request = savedRequest ?? {
      id: randomUUID(),
      previous: this.suggestedProject?.leadId ?? null,
      reuse_empty: false,
      ...body,
    };
    this.isCreating = true;
    if (!this.areas.navigation.selectedId)
      this.areas.delivery.editDraft(
        this.areas.scope.services.drafts.getDraft("new"),
        request.id,
      );
    try {
      chatCreations.saveChatCreation(key, request);
      this.reloadPending();
      const receipt = yield* flowValue(
        untilAnswered(() =>
          api.post("/api/leads", request, { timeoutMs: 60000 }),
        ),
      );
      if (
        !chatCreations.confirmChatCreationResponse(key, request.id, receipt.id)
      )
        throw new Error("The server returned another chat identity.");
      if (!this.isCurrent(intent, generation)) return null;
      this.reloadPending();
      this.acceptCreated(receipt, intent, generation);
      return receipt.id;
    } catch (failure) {
      const settled = chatCreations.settleFailedChatCreation(
        key,
        request.id,
        failure,
        errorText(failure),
      );
      if (this.isCurrent(intent, generation)) {
        if (settled) this.reloadPending();
        this.areas.notices.notify(errorText(failure));
      }
      return null;
    } finally {
      if (generation === this.generation) this.isCreating = false;
    }
  });
  get hasUnreplicatedSelection() {
    return this.unreplicatedCreatedAgents.some(
      (agent) => agent.id === this.areas.navigation.selectedId,
    );
  }
  get unreplicatedCreatedAgents() {
    return this.receipts.scope === this.areas.startup.creationScope
      ? this.receipts.agents.filter(
          (agent) => !this.areas.state?.agents.has(agent.id),
        )
      : [];
  }
  lookup(id: string) {
    return (
      this.areas.state?.agents.get(id) ??
      this.unreplicatedCreatedAgents.find((agent) => agent.id === id)
    );
  }
  forget(ids: readonly string[]) {
    this.receipts = {
      ...this.receipts,
      agents: this.receipts.agents.filter((agent) => !ids.includes(agent.id)),
    };
    this.persist();
  }
  acknowledgeReplication() {
    const agents = this.unreplicatedCreatedAgents;
    if (
      !this.areas.startup.creationScope ||
      (this.receipts.scope === this.areas.startup.creationScope &&
        agents.length === this.receipts.agents.length)
    )
      return;
    this.receipts = { scope: this.areas.startup.creationScope, agents };
    this.persist();
  }
  private persist() {
    this.areas.scope.services.api.save("codex-confirmed-chats", this.receipts);
  }
}

/** Dialog lifetime owns the immutable connection read and model draft. */
export class ProjectServerSelection {
  selected = "";
  read = new ProjectServerRead();
  services: ServerServices | undefined;
  model: NewChatModelStore | undefined;
  private foreign = false;
  private stop = () => {};
  readonly project;
  readonly choices;
  readonly servicesFor;
  readonly create;
  constructor(
    project: LocationProjectView,
    choices: () => ProjectServerChoice[],
    lastServer: string | undefined,
    servicesFor: (id: string) => ServerServices | undefined,
    create = createServerServices,
  ) {
    this.project = project;
    this.choices = choices;
    this.servicesFor = servicesFor;
    this.create = create;
    makeAutoObservable<this, "stop">(
      this,
      {
        project: false,
        choices: false,
        servicesFor: false,
        create: false,
        stop: false,
        view: false,
      },
      { autoBind: true, deep: false },
    );
    const available = (row: ProjectServerChoice) =>
      !row.disabled && !!projectLocationOn(project, row);
    const previous = choices().find(
      (row) => row.id === lastServer || row.serverId === lastServer,
    );
    this.selected =
      (previous && available(previous) ? previous : choices().find(available))
        ?.id || "";
    this.stop = reaction(
      () => {
        const connection = this.choices().find(
          (row) => row.id === this.selected,
        )?.connection;
        return [
          this.selected,
          connection?.id,
          connection?.origin,
          connection?.credentialId,
          this.servicesFor(this.selected),
        ];
      },
      this.attach,
      { fireImmediately: true, equals: compareShallow },
    );
  }
  get capabilities() {
    const server = this.choices().find((row) => row.id === this.selected),
      known =
        !!server?.system &&
        server.nativeLayr !== undefined &&
        server.defaultWorkspaceMode !== undefined;
    const fetched =
      server && !server.disabled && !known ? this.read.value : undefined;
    const nativeLayr = server?.nativeLayr ?? fetched?.nativeLayr,
      reported = server?.defaultWorkspaceMode ?? fetched?.defaultWorkspaceMode;
    return {
      server,
      system: server?.system ?? fetched?.system,
      nativeLayr,
      nativeAvailable: !!nativeLayr && nativeLayr.status !== "notInstalled",
      nativeReason: server?.nativeLayrMessage ?? fetched?.nativeLayrMessage,
      defaultMode:
        reported === "layr-native"
          ? ("layr-native" as const)
          : reported
            ? ("worktree" as const)
            : undefined,
      platformError: known || server?.disabled ? "" : this.read.error,
    };
  }
  view(
    project: LocationProjectView,
    chosenMode: NewChatSettings["workspaceMode"] | undefined,
  ) {
    const platform = this.capabilities,
      location = platform.server && projectLocationOn(project, platform.server),
      worktreeAllowed = projectAllowsWorktree(project),
      mode =
        platform.defaultMode &&
        (chosenMode &&
        (chosenMode !== "layr-native" || platform.nativeAvailable)
          ? chosenMode
          : platform.defaultMode),
      models = this.model?.selection;
    const ready =
      !!location &&
      !!models &&
      !!platform.system &&
      !!mode &&
      (mode !== "layr-native" || platform.nativeAvailable) &&
      (mode !== "worktree" || worktreeAllowed) &&
      !platform.platformError &&
      !platform.server?.disabled;
    return {
      ...platform,
      selected: this.selected,
      mode,
      worktreeAllowed,
      selection:
        ready && location && models && mode
          ? {
              location: { ...location, serverId: this.selected },
              settings: { ...models, workspaceMode: mode },
            }
          : null,
    };
  }
  get key() {
    const connection = this.choices().find(
      (row) => row.id === this.selected,
    )?.connection;
    return JSON.stringify([
      this.selected,
      connection?.id,
      connection?.origin,
      connection?.credentialId,
    ]);
  }
  select(id: string) {
    this.selected = id;
  }
  attach() {
    this.closeServices();
    const id = this.selected;
    const server = this.choices().find((row) => row.id === id);
    this.read = new ProjectServerRead(server?.connection);
    this.services = this.servicesFor(id);
    this.foreign = !this.services && !!server?.connection;
    if (this.foreign) this.services = this.create(server!.connection!);
    const location = server && projectLocationOn(this.project, server);
    this.model =
      this.services && location
        ? new NewChatModelStore(
            this.services,
            this.project.id,
            location.path,
            this.services.areas,
          )
        : undefined;
  }
  dispose() {
    this.stop();
    this.closeServices();
  }
  closeServices() {
    if (this.foreign) this.services?.close();
    this.foreign = false;
    this.services = undefined;
    this.model = undefined;
  }
}

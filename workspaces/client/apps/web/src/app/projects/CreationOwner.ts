import { makeAutoObservable, reaction, runInAction } from "mobx";
import { MarkUnreadEligibility } from "../../generated/api.ts";
import {
  errorText,
  untilAnswered,
  type PostBody,
  type PostResult,
} from "../../api.ts";
import type { Json } from "../../types.ts";
import type { NewChatSettings } from "./newChatSettings.ts";
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
export type ProjectBinding = { projectId: string; homeServerId: string };
/** Durable creation identity is settled before any scope-sensitive UI action. */
export class CreationOwner {
  readonly areas;
  private receipts: { scope: string; agents: AgentModel[] };
  private pending: Json[] = [];
  private disposed = false;
  private generation = 0;
  private stop = () => {};
  isCreating = false;
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
      { areas: false, stop: false },
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
      }),
      ({ scope }, previous) => {
        if (scope !== previous?.scope) {
          this.generation++;
          this.isCreating = false;
          this.reloadPending();
          this.closeShared();
          if (api.saved(`studio-radio-create:${scope}`, null))
            this.openShared();
        }
        this.acknowledgeReplication();
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
  async ensureConversation(settings?: NewChatSettings) {
    if (this.areas.navigation.selectedId)
      return this.areas.navigation.selectedId;
    const created = await this.newChat(
      undefined,
      undefined,
      undefined,
      undefined,
      settings,
    );
    return created ?? undefined;
  }
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
  openShared(initialPath?: string) {
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
  async newChat(
    cwd?: string,
    folder?: string,
    retryId?: string,
    binding?: ProjectBinding,
    settings?: NewChatSettings,
  ): Promise<
    | string
    | null
    | {
        kind: "needs-project";
        cwd: string | undefined;
        folder: string | undefined;
        binding: ProjectBinding | undefined;
      }
  > {
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
      const receipt = await untilAnswered(() =>
        api.post("/api/leads", request, { timeoutMs: 60000 }),
      );
      if (
        !chatCreations.confirmChatCreationResponse(key, request.id, receipt.id)
      )
        throw new Error("The server returned another chat identity.");
      runInAction(() => {
        if (!this.isCurrent(intent, generation)) return;
        this.reloadPending();
        this.acceptCreated(receipt, intent, generation);
      });
      if (!this.isCurrent(intent, generation)) return null;
      return receipt.id;
    } catch (failure) {
      const settled = chatCreations.settleFailedChatCreation(
        key,
        request.id,
        failure,
        errorText(failure),
      );
      runInAction(() => {
        if (!this.isCurrent(intent, generation)) return;
        if (settled) {
          this.reloadPending();
        }
        this.areas.notices.notify(errorText(failure));
      });
      return null;
    } finally {
      runInAction(() => {
        if (generation === this.generation) this.isCreating = false;
      });
    }
  }
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

import { makeAutoObservable } from "mobx";
import { requestId } from "studio-claude-bridge/request-id.ts";
import {
  ApiError,
  errorText,
  type PostBody,
  type PostResult,
} from "../../../api.ts";
import { AgentMode } from "../../../generated/api.ts";
import { outboxRequests } from "../../../browserPersistence.ts";
import {
  DEFAULT_SUBAGENT_CONCURRENCY,
  MAX_SUBAGENT_CONCURRENCY as MAX_CONCURRENCY,
  MIN_SUBAGENT_CONCURRENCY as MIN_CONCURRENCY,
} from "../../../generated/apiSchema.ts";
import { agentId } from "../../../generated/observableResources.ts";
import { resourceData } from "../../../store/runtime.ts";
import type { Agent } from "../../../types.ts";
import type {
  useServerApi,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
type PendingRequest = PostBody<"/api/conversation">;
type Stored = { pending?: PendingRequest | undefined };
export const subagentsOff = (lead: Pick<Agent, "concurrency">) =>
  lead.concurrency === MIN_CONCURRENCY;
const validConcurrency = (value: unknown): value is number =>
  Number.isSafeInteger(value) &&
  Number(value) >= MIN_CONCURRENCY &&
  Number(value) <= MAX_CONCURRENCY;
const validRevision = (value: unknown): value is number =>
  Number.isSafeInteger(value) && Number(value) >= 0;
const pendingTarget = (
  request: PendingRequest | undefined,
): number | undefined =>
  request
    ? (request.subagent_concurrency ??
      (request.agent_mode === AgentMode["single"]
        ? MIN_CONCURRENCY
        : DEFAULT_SUBAGENT_CONCURRENCY))
    : undefined;
const validPending = (
  value: unknown,
  leadId: string,
): value is PendingRequest => {
  if (!value || typeof value !== "object") return false;
  const request = value as Record<string, unknown>;
  if (
    request.id !== leadId ||
    !validRevision(request.expected_mode_revision) ||
    typeof request.request_id !== "string" ||
    !request.request_id
  )
    return false;
  return "subagent_concurrency" in request
    ? validConcurrency(request.subagent_concurrency)
    : request.agent_mode === AgentMode["single"] ||
        request.agent_mode === AgentMode["multi"];
};
export function decodeStoredConcurrency(
  raw: string | null,
  leadId: string,
): Stored {
  let value: unknown;
  try {
    value = raw === null ? {} : JSON.parse(raw);
  } catch {
    throw new Error("The saved concurrency request cannot be read.");
  }
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("The saved concurrency request cannot be read.");
  const pending = "pending" in value ? value.pending : undefined;
  if (pending !== undefined && !validPending(pending, leadId))
    throw new Error("The saved concurrency request cannot be read.");
  return pending === undefined ? {} : { pending };
}

export class ConcurrencyStore {
  lead: Agent;
  owner: ReturnType<typeof useServerStore>;
  api: Pick<ReturnType<typeof useServerApi>, "post">;
  outbox: ReturnType<typeof outboxRequests>;
  suffix: string;
  workspaceId: string;
  pending: PendingRequest | undefined;
  edit: number | string | undefined;
  error = "";
  storageError = "";
  saving = false;
  constructor(
    lead: Agent,
    owner: ReturnType<typeof useServerStore>,
    api: Pick<ReturnType<typeof useServerApi>, "post">,
    storage: Storage,
    scope: string,
    workspaceId: string,
  ) {
    this.lead = lead;
    this.owner = owner;
    this.api = api;
    this.outbox = outboxRequests(storage);
    this.suffix = scope;
    this.workspaceId = workspaceId;
    try {
      this.pending = this.read().pending;
    } catch (cause) {
      this.error = this.storageError = errorText(cause);
    }
    makeAutoObservable(
      this,
      { lead: false, owner: false, api: false, outbox: false },
      { autoBind: true },
    );
  }
  get confirmed() {
    return resourceData(this.owner.agentDetail(agentId(this.lead.id)).current);
  }
  get supported() {
    return (
      this.confirmed?.concurrency != null &&
      this.confirmed.agentModeRevision != null
    );
  }
  get draft() {
    return (
      this.edit ??
      pendingTarget(this.pending) ??
      this.confirmed?.concurrency ??
      ""
    );
  }
  get mode() {
    return this.supported
      ? this.confirmed?.concurrency === 0
        ? "single"
        : "multi"
      : undefined;
  }
  get disabled() {
    return (
      !this.supported ||
      !this.workspaceId ||
      this.saving ||
      !!this.pending ||
      !!this.storageError
    );
  }
  read() {
    try {
      return decodeStoredConcurrency(
        this.outbox.get("agentMode", this.suffix),
        this.lead.id,
      );
    } catch (cause) {
      this.storageError = errorText(cause);
      throw cause;
    }
  }
  write(pending?: PendingRequest) {
    if (pending)
      this.outbox.set("agentMode", this.suffix, JSON.stringify({ pending }));
    else this.outbox.remove("agentMode", this.suffix);
  }
  setDraft(value: number | string) {
    this.edit = value;
    this.error = "";
  }
  discard() {
    try {
      this.write();
      this.pending = undefined;
      this.edit = undefined;
      this.error = this.storageError = "";
    } catch (cause) {
      this.error = errorText(cause);
    }
  }
  *submit(
    target = this.draft,
  ): Generator<
    Promise<PostResult<"/api/conversation">>,
    void,
    PostResult<"/api/conversation">
  > {
    const confirmed = this.confirmed;
    const concurrency = confirmed?.concurrency,
      revision = confirmed?.agentModeRevision;
    if (
      concurrency == null ||
      revision == null ||
      this.saving ||
      this.storageError ||
      !this.workspaceId
    )
      return;
    if (
      !this.pending &&
      (!validConcurrency(target) || target === concurrency)
    ) {
      if (!validConcurrency(target))
        this.error = `Enter a whole number from ${MIN_CONCURRENCY} to ${MAX_CONCURRENCY}.`;
      else this.edit = undefined;
      return;
    }
    const request = this.pending ?? {
      id: this.lead.id,
      subagent_concurrency: Number(target),
      expected_mode_revision: revision,
      request_id: requestId(),
      expected_account_key: this.lead.accountKey || "default",
    };
    try {
      this.write(request);
    } catch {
      this.error =
        "Cannot save the concurrency request on this device. Free browser storage and retry.";
      return;
    }
    this.pending = request;
    this.saving = true;
    this.error = "";
    try {
      yield this.api.post("/api/conversation", request, {
        workspaceId: this.workspaceId,
        timeoutMs: 15000,
      });
      const disk = this.read();
      const pending =
        disk.pending?.request_id !== request.request_id
          ? disk.pending
          : undefined;
      this.write(pending);
      this.pending = pending;
      this.edit = undefined;
    } catch (cause) {
      if (cause instanceof ApiError && cause.outcome === "not_applied") {
        try {
          const disk = this.read();
          const pending =
            disk.pending?.request_id !== request.request_id
              ? disk.pending
              : undefined;
          this.write(pending);
          this.pending = pending;
          this.edit = undefined;
        } catch (failure) {
          this.error = errorText(failure);
          return;
        }
      }
      this.error = errorText(cause);
    } finally {
      this.saving = false;
    }
  }
}

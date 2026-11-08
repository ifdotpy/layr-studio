import { jsonObject } from "../../usage/accountUsage.ts";
import type { ServerAreas } from "../ServerAreasProvider.tsx";
import { outboxRequests } from "../../browserPersistence";
import { randomUUID } from "../../uuid";
import { makeAutoObservable, runInAction } from "mobx";
import { ApiError } from "../../api";
import type { PostBody } from "../../api";
import type { Agent } from "../../types";
type Request = Omit<PostBody<"/api/action">, "action"> & {
  action: "compact" | "review";
};
type Action = Request["action"];

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object";
}

function isActionRequest(value: unknown): value is Request {
  if (!isRecord(value)) return false;
  const request = value;
  if (
    typeof request.id !== "string" ||
    typeof request.request_id !== "string" ||
    !request.request_id ||
    (request.action !== "compact" && request.action !== "review") ||
    !isRecord(request.context)
  )
    return false;
  const context = request.context;
  return (
    typeof context.accountKey === "string" &&
    (context.threadId === undefined || typeof context.threadId === "string") &&
    (context.epoch === undefined || typeof context.epoch === "number")
  );
}

export function isDefinitivelyNotApplied(error: unknown): boolean {
  if (
    !(error instanceof ApiError) ||
    !error.response ||
    typeof error.response !== "object"
  )
    return false;
  return (
    "outcome" in error.response && error.response.outcome === "not_applied"
  );
}

export class NativeActionOwner {
  active = false;
  disposed = false;
  revision = 0;
  readonly areas;
  constructor(areas: ServerAreas) {
    this.areas = areas;
    makeAutoObservable(
      this,
      { areas: false, disposed: false },
      { autoBind: true },
    );
  }
  start() {
    this.disposed = false;
    return () => {
      this.disposed = true;
    };
  }
  read(id: string): Request | null {
    const workspace = this.areas.startup.creationScope;
    return workspace ? this.readKey(id, `${workspace}:${id}`) : null;
  }
  readKey(id: string, key: string): Request | null {
    const raw = outboxRequests(this.areas.scope.services.storage).get(
      "nativeAction",
      key,
    );
    if (!raw) return null;
    const request: unknown = JSON.parse(raw);
    if (!isActionRequest(request) || request.id !== id)
      throw new Error(
        "The saved action request is invalid. Restore browser storage before this action.",
      );
    return request;
  }
  async submit(agent: Agent, action: Action) {
    const workspace = this.areas.startup.creationScope;
    const workspaceId = this.areas.startup.workspaceId;
    const { post } = this.areas.scope.services.api;
    const notify = (message: string) => {
      if (!this.disposed) this.areas.notices.notify(message);
    };
    const outbox = outboxRequests(this.areas.scope.services.storage);
    const key = (id: string) => `${workspace}:${id}`;
    if (!workspace) throw new Error("Wait for the workspace to load.");
    const existing = this.read(agent.id);
    if (existing && existing.action !== action)
      throw new Error("Check the saved action request before another action.");
    const request: Request = existing || {
      id: agent.id,
      action,
      request_id: randomUUID(),
      context: {
        accountKey: agent.accountKey || "default",
        threadId: agent.threadId,
        epoch: agent.epoch,
      },
    };
    // Storage must succeed before HTTP. Keep the exact account, thread and epoch on retry.
    outbox.set("nativeAction", key(agent.id), JSON.stringify(request));
    runInAction(() => {
      this.revision++;
    });
    this.active = true;
    const acknowledge = () => {
      if (
        this.readKey(agent.id, key(agent.id))?.request_id === request.request_id
      )
        outbox.remove("nativeAction", key(agent.id));
      runInAction(() => {
        this.revision++;
      });
    };
    try {
      const response = await post("/api/action", request, {
        workspaceId,
        timeoutMs: 15000,
      });
      if (response.receipt?.requestId !== request.request_id)
        throw new Error(
          "The action reply has no matching receipt. Check the saved request.",
        );
      acknowledge();
      const resultError = jsonObject(response.result)?.error;
      const error =
        response.error ||
        response.outcome?.error ||
        (typeof resultError === "string" ? resultError : undefined);
      if (error) notify(error);
      else if (response.replayed)
        notify("This action request was already saved. No new action starts.");
      return response;
    } catch (error) {
      if (isDefinitivelyNotApplied(error)) acknowledge();
      throw error;
    } finally {
      runInAction(() => {
        this.active = false;
      });
    }
  }
  pending(id: string) {
    void this.revision;
    try {
      return this.read(id);
    } catch {
      return null;
    }
  }
}

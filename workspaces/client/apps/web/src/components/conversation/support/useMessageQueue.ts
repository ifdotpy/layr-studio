import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { outboxRequests } from "../../../browserPersistence.ts";
import {
  useServerApi,
  useServerResources,
  useServerStore,
  useServerStorage,
  useServerLocalDrafts,
  useServerStorageEventKey,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useEffect, useMemo, useState } from "react";
import {
  makeAutoObservable,
  observableRef,
  computed,
  compareShallow,
} from "mobx";
import { ApiError, errorText } from "../../../api.ts";
import type { QueueItemDto, QueueView } from "../../../api.ts";
import { onResume } from "../../../sync/resume.ts";
import type { paths } from "../../../generated/api.ts";
import { resourceData } from "../../../store/runtime.ts";
import type { DeepReadonly } from "../../../deepReadonly.ts";

type QueueMutation =
  paths["/api/queue"]["post"]["requestBody"]["content"]["application/json"];
type QueueChange<T = QueueMutation> = T extends unknown
  ? Omit<T, "agent" | "expected_revision" | "request_id">
  : never;

const emptyQueue: QueueView = {
  items: [],
  revision: "",
  capabilities: { reorder: false, receipts: false },
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStringArray(value: unknown): value is string[] {
  return (
    Array.isArray(value) && value.every((item) => typeof item === "string")
  );
}

function readStoredQueueRequest(storage: Storage, suffix: string): unknown {
  try {
    const stored = outboxRequests(storage).get("queueChange", suffix);
    if (stored === null) return null;
    try {
      const request: unknown = JSON.parse(stored);
      return request;
    } catch {
      return stored;
    }
  } catch {
    return "";
  }
}

function isQueueMutation(value: unknown): value is QueueMutation {
  if (
    !isRecord(value) ||
    "message_id" in value ||
    typeof value.agent !== "string" ||
    typeof value.request_id !== "string" ||
    typeof value.expected_revision !== "string"
  )
    return false;
  if (value.action === "reorder") return isStringArray(value.ordered_ids);
  if (typeof value.id !== "string") return false;
  return (
    value.action === "first" ||
    ((value.action === "send_now" || value.action === "edit") &&
      typeof value.expectedText === "string" &&
      (value.action !== "edit" || typeof value.text === "string"))
  );
}

type QueueOptions = {
  id: string | null;
  enabled: boolean;
  scope: string;
  workspaceId?: string | undefined;
  observed?: ((ids: string[]) => void) | undefined;
  edited?: ((id: string, text: string) => void) | undefined;
  refresh: () => Promise<void>;
};

export function useMessageQueue(p: QueueOptions) {
  const store = useServerStore();
  const resources = useServerResources();
  const api = useServerApi();
  const { syncPost } = api;
  const storage = useServerStorage();
  const { updateQueueRequest } = useServerLocalDrafts();
  const serverStorageEventKey = useServerStorageEventKey();
  const [locks] = useState(() => new Set<string>());
  const queue = useMemo(() => {
    const suffix = `${p.scope}:${p.id}`,
      key = outboxRequests(storage).key("queueChange", suffix);
    const queue = {
      key,
      active: true,
      error: "",
      busy: false,
      pending: readStoredQueueRequest(storage, suffix),
      commands: api.pending.list(),
      get current() {
        return p.enabled && p.id ? store.state().current : undefined;
      },
      get view() {
        return (
          (this.current &&
            p.id &&
            resourceData(this.current)?.queues.get(p.id)) ||
          emptyQueue
        );
      },
      get items() {
        return this.view.items.filter(
          (item) =>
            !this.commands.some(
              (entry) =>
                entry.kind === "withdraw" &&
                entry.target === p.id &&
                entry.message === item.id,
            ),
        );
      },
      get loading() {
        return !["live", "stale"].includes(this.current?.status || "");
      },
      get canReorder() {
        return !!this.view.capabilities?.reorder;
      },
      setPending(request: unknown) {
        this.pending = request;
      },
      finish() {
        locks.delete(key);
        this.busy = false;
      },
      syncPending() {
        this.setPending(readStoredQueueRequest(storage, suffix));
      },
      syncCommands() {
        this.commands = api.pending.list();
      },
      async reload() {
        if (!p.enabled || !p.id) return;
        // Queue revisions are opaque; only the state stream can order queue facts.
        resources.replaceStream();
      },
      async clearRequest(request: QueueMutation) {
        const next = await updateQueueRequest<unknown>(
          suffix,
          null,
          (current) =>
            isQueueMutation(current) &&
            current.request_id === request.request_id
              ? null
              : current,
        );
        if (this.active) this.setPending(next);
      },
      async run(operation: () => Promise<void>) {
        if (locks.has(key))
          throw new Error("Wait for the current queue change.");
        locks.add(key);
        this.busy = true;
        try {
          await operation();
          if (this.active) {
            await this.reload().catch(this.fail);
            void p.refresh().catch(() => {});
          }
        } catch (error) {
          if (this.active) await this.reload().catch(() => {});
          throw error;
        } finally {
          this.finish();
        }
      },
      async execute(request: QueueMutation) {
        if (!p.id || request.agent !== p.id)
          throw new Error("The queue belongs to another chat.");
        await this.run(async () => {
          try {
            await syncPost("/api/queue", request, {
              workspaceId: p.workspaceId,
            });
            await this.clearRequest(request);
            if (request.action === "edit" && request.id)
              p.edited?.(request.id, request.text);
          } catch (error) {
            if (
              error instanceof ApiError &&
              [400, 401, 403, 404].includes(error.status)
            )
              await this.clearRequest(request);
            throw error;
          }
        });
      },
      fail(error: unknown) {
        this.error = errorText(error);
      },
      async mutate(change: QueueChange) {
        if (!p.id) throw new Error("The queue belongs to another chat.");
        const existing = readStoredQueueRequest(storage, suffix);
        this.pending = existing;
        if (existing !== null)
          throw new Error(
            "Retry the previous queue change before making another change.",
          );
        if (!this.view.capabilities?.receipts || !this.view.revision)
          throw new Error(
            "The queue is still loading or the server needs an update.",
          );
        const request = {
          ...change,
          agent: p.id,
          expected_revision: this.view.revision,
          request_id: randomUUID(),
        } satisfies QueueMutation;
        // Save before HTTP so a lost response or reload reuses the same operation.
        await updateQueueRequest<unknown>(suffix, null, (current) => {
          if (current !== null)
            throw new Error(
              "Retry the previous queue change before making another change.",
            );
          return request;
        });
        this.setPending(request);
        return this.execute(request);
      },
      async retry() {
        const request = readStoredQueueRequest(storage, suffix);
        this.pending = request;
        if (request !== null) {
          if (!isQueueMutation(request))
            throw new Error("The saved queue change cannot be verified.");
          await this.execute(request);
        }
      },
      async edit(item: DeepReadonly<QueueItemDto>, text: string) {
        const current = this.view.items.find((row) => row.id === item.id);
        await this.mutate({
          action: "edit",
          id: item.id,
          text,
          expectedText:
            current?.text === text.trim() ? current.text : item.text,
        });
      },
      async cancel(item: DeepReadonly<QueueItemDto>) {
        const id = p.id;
        if (!id) throw new Error("The queue belongs to another chat.");
        await this.run(async () => {
          await api.command.withdraw(id, item.id, {
            workspaceId: p.workspaceId,
          });
          p.observed?.([item.id]);
        });
      },
      sendNow(item: DeepReadonly<QueueItemDto>) {
        return this.mutate({
          action: "send_now",
          id: item.id,
          expectedText: item.text,
        });
      },
      reorder(ids: string[]) {
        return this.mutate({ action: "reorder", ordered_ids: ids });
      },
    };
    return makeAutoObservable(
      queue,
      {
        active: false,
        pending: observableRef,
        commands: observableRef,
        items: computed({ equals: compareShallow }),
      },
      { autoBind: true },
    );
  }, [
    p.scope,
    p.id,
    p.enabled,
    p.workspaceId,
    store,
    resources,
    api,
    storage,
    updateQueueRequest,
    locks,
  ]);
  useEffect(() => {
    queue.active = true;
    const changed = (event: StorageEvent) => {
      if (
        serverStorageEventKey(event) === queue.key ||
        serverStorageEventKey(event) === null
      )
        queue.syncPending();
    };
    window.addEventListener("storage", changed);
    const stopResume = onResume(queue.syncPending);
    const stopCommands = api.pending.subscribe(queue.syncCommands);
    queue.syncPending();
    queue.syncCommands();
    return () => {
      queue.active = false;
      window.removeEventListener("storage", changed);
      stopResume();
      stopCommands();
    };
  }, [queue, api, serverStorageEventKey]);
  return queue;
}

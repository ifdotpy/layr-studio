import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { outboxRequests } from "../../browserPersistence.ts";
import {
  flow,
  makeAutoObservable,
  observableRef,
  computed,
  compareShallow,
} from "mobx";
import { ApiError, errorText } from "../../api.ts";
import type { ServerApi, QueueView, QueueItemDto } from "../../api.ts";
import type { createServerStore } from "../../store/server.ts";
import type { createLocalDraftStorage } from "../../sync/localDraft.ts";
import type { paths } from "../../generated/api.ts";
import { resourceData } from "../../store/runtime.ts";
import type { DeepReadonly } from "../../deepReadonly.ts";
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

function readStoredQueueRequest(
  storage: Storage,
  suffix: string,
): { pending: QueueMutation | null; error: string } {
  try {
    const pending: QueueMutation | null = JSON.parse(
      outboxRequests(storage).get("queueChange", suffix) ?? "null",
    );
    return { pending, error: "" };
  } catch (error) {
    return {
      pending: null,
      error: `The saved queue change could not be read. ${errorText(error)}`,
    };
  }
}

export type QueueOptions = {
  id: string | null;
  enabled: boolean;
  scope: string;
  workspaceId?: string | undefined;
  observed?: ((ids: string[]) => void) | undefined;
  edited?: ((id: string, text: string) => void) | undefined;
};

export function createMessageQueue(
  p: QueueOptions,
  store: ReturnType<typeof createServerStore>,
  api: ServerApi,
  storage: Storage,
  updateQueueRequest: ReturnType<
    typeof createLocalDraftStorage
  >["updateQueueRequest"],
  locks: Set<string>,
) {
  const { syncPost } = api;
  const suffix = `${p.scope}:${p.id}`,
    key = outboxRequests(storage).key("queueChange", suffix);
  const stored = readStoredQueueRequest(storage, suffix);
  const queue = makeAutoObservable(
    {
      key,
      active: true,
      error: stored.error,
      blocked: !!stored.error,
      busy: false,
      pending: stored.pending,
      get commands() {
        return api.pending.list();
      },
      get current(): ReturnType<typeof store.state>["current"] | undefined {
        return p.enabled && p.id ? store.state().current : undefined;
      },
      get view(): DeepReadonly<QueueView> {
        return (
          (this.current &&
            p.id &&
            resourceData(store.state().current)?.queues.get(p.id)) ||
          emptyQueue
        );
      },
      get items(): DeepReadonly<QueueItemDto>[] {
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
        return !this.blocked && !!this.view.capabilities?.reorder;
      },
      syncPending() {
        const stored = readStoredQueueRequest(storage, suffix);
        if (stored.error || this.blocked) this.error = stored.error;
        this.blocked = !!stored.error;
        this.pending = stored.pending;
      },
    },
    {
      active: false,
      pending: observableRef,
      items: computed({ equals: compareShallow }),
    },
    { autoBind: true },
  );
  const clearRequest = flow(function* (request: QueueMutation) {
    const next: QueueMutation | null =
      yield updateQueueRequest<QueueMutation | null>(suffix, null, (current) =>
        current?.request_id === request.request_id ? null : current,
      );
    if (queue.active) queue.pending = next;
  });
  const run = flow(function* (operation: () => Promise<void>) {
    if (queue.blocked) throw new Error(queue.error);
    if (locks.has(key)) throw new Error("Wait for the current queue change.");
    locks.add(key);
    queue.busy = true;
    try {
      yield operation();
    } finally {
      locks.delete(key);
      queue.busy = false;
    }
  });
  const execute = flow(function* (request: QueueMutation) {
    if (!p.id || request.agent !== p.id)
      throw new Error("The queue belongs to another chat.");
    yield run(
      flow(function* () {
        try {
          yield syncPost("/api/queue", request, {
            workspaceId: p.workspaceId,
          });
          yield clearRequest(request);
          if (request.action === "edit" && request.id)
            p.edited?.(request.id, request.text);
        } catch (error) {
          if (
            error instanceof ApiError &&
            [400, 401, 403, 404].includes(error.status)
          )
            yield clearRequest(request);
          throw error;
        }
      }),
    );
  });
  const mutate = flow(function* (change: QueueChange) {
    if (!p.id) throw new Error("The queue belongs to another chat.");
    queue.syncPending();
    if (queue.blocked) throw new Error(queue.error);
    const existing = queue.pending;
    if (existing !== null)
      throw new Error(
        "Retry the previous queue change before making another change.",
      );
    if (!queue.view.capabilities?.receipts || !queue.view.revision)
      throw new Error(
        "The queue is still loading or the server needs an update.",
      );
    const request = {
      ...change,
      agent: p.id,
      expected_revision: queue.view.revision,
      request_id: randomUUID(),
    } satisfies QueueMutation;
    // Save before HTTP so a lost response or reload reuses the same operation.
    yield updateQueueRequest<unknown>(suffix, null, (current) => {
      if (current !== null)
        throw new Error(
          "Retry the previous queue change before making another change.",
        );
      return request;
    });
    queue.pending = request;
    yield execute(request);
  });
  const retry = flow(function* () {
    queue.syncPending();
    if (queue.blocked) throw new Error(queue.error);
    const request = queue.pending;
    if (request !== null) {
      yield execute(request);
    }
  });
  const edit = flow(function* (item: DeepReadonly<QueueItemDto>, text: string) {
    const current = queue.view.items.find((row) => row.id === item.id);
    yield mutate({
      action: "edit",
      id: item.id,
      text,
      expectedText: current?.text === text.trim() ? current.text : item.text,
    });
  });
  const cancel = flow(function* (item: DeepReadonly<QueueItemDto>) {
    const id = p.id;
    if (!id) throw new Error("The queue belongs to another chat.");
    yield run(
      flow(function* () {
        yield api.command.withdraw(id, item.id, {
          workspaceId: p.workspaceId,
        });
        p.observed?.([item.id]);
      }),
    );
  });
  return Object.assign(queue, {
    retry,
    edit,
    cancel,
    sendNow(item: DeepReadonly<QueueItemDto>) {
      return mutate({
        action: "send_now",
        id: item.id,
        expectedText: item.text,
      });
    },
    reorder(ids: string[]) {
      return mutate({ action: "reorder", ordered_ids: ids });
    },
  });
}

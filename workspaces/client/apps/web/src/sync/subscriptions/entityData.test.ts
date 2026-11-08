import { autorun } from "mobx";
import { createServerStore } from "../../store/server.ts";
import { taskId } from "../../generated/observableResources.ts";
import { createSubscriptions } from "./module.ts";
import type { ResourceVersion } from "./contracts.ts";
import { expect, it, vi } from "vitest";
import { createEntityDataObserver } from "./entityData.ts";
import type { createSyncClient } from "../client.ts";
import type { ObserveSubscriptionData } from "./module.ts";
vi.mock("../client.ts", () => ({ syncDatabase: vi.fn() }));

// Published cache rows are shared between observers. Consumers must copy before editing.
function publicationTypeWitness(observe: ObserveSubscriptionData) {
  observe(
    { kind: "work", agentId: "lead" },
    (data) => {
      if (!data) return;
      // @ts-expect-error Shared subscription arrays cannot be edited by consumers.
      data.items.push(data.items[0]!);
      const first = data.items[0];
      if (first) {
        // @ts-expect-error Nested cache values are also readonly.
        first.title = "Changed by a consumer";
      }
      // @ts-expect-error A work resource never publishes a process list.
      const processes: { processes: unknown[] } = data;
      void processes;
    },
    () => {},
  );
}
void publicationTypeWitness;

it("selects one visible agent detail before decoding and releases its observer", async () => {
  let emit!: (rows: { id: string; toJSON: () => unknown }[]) => void;
  const unsubscribe = vi.fn();
  const find = vi.fn((query?: { selector?: { id?: string } }) => ({
    $: {
      subscribe: ({
        next,
      }: {
        next: (rows: { toJSON: () => unknown }[]) => void;
      }) => {
        emit = (rows) =>
          next(
            query?.selector?.id
              ? rows.filter((row) => row.id === query.selector!.id)
              : rows,
          );
        return { unsubscribe };
      },
    },
  }));
  const observe = createEntityDataObserver((async () => ({
    db: { projections: { find } },
  })) as unknown as ReturnType<typeof createSyncClient>["syncDatabase"]);
  const accept = vi.fn();
  const stop = observe(
    { kind: "agentDetail", agentId: "visible" },
    accept,
    vi.fn(),
  );
  await vi.waitFor(() => expect(emit).toBeTypeOf("function"));
  const visible = {
    id: "entity:agentDetail:visible",
    toJSON: vi.fn(() => ({
      id: "entity:agentDetail:visible",
      seq: 1,
      entity: {
        collection: "agentDetail",
        id: "visible",
        value: { id: "visible", overview: { task: "Current work" } },
      },
    })),
  };
  const unrelated = {
    id: "entity:workResult:other:0",
    toJSON: vi.fn(() => ({ id: "entity:workResult:other:0" })),
  };
  emit([visible, unrelated]);
  expect(find).toHaveBeenCalledExactlyOnceWith({
    selector: { id: "entity:agentDetail:visible" },
  });
  expect(visible.toJSON).toHaveBeenCalledTimes(1);
  expect(unrelated.toJSON).not.toHaveBeenCalled();
  expect(accept).toHaveBeenCalledTimes(1);
  emit([visible, unrelated]);
  expect(accept).toHaveBeenCalledTimes(1);
  stop();
  expect(unsubscribe).toHaveBeenCalledOnce();
});

it("ignores offscreen workspace changes and preserves unchanged typed row identities", async () => {
  let emit!: (rows: { toJSON: () => unknown }[]) => void;
  const observe = createEntityDataObserver((async () => ({
    db: {
      projections: {
        find: () => ({
          $: {
            subscribe: ({ next }: { next: typeof emit }) => {
              emit = next;
              return { unsubscribe() {} };
            },
          },
        }),
      },
    },
  })) as unknown as ReturnType<typeof createSyncClient>["syncDatabase"]);
  const accept = vi.fn();
  const stop = observe({ kind: "work", agentId: "lead" }, accept, vi.fn());
  await vi.waitFor(() => expect(emit).toBeTypeOf("function"));
  const row = (id: string, rootId: string, seq: number) => ({
    id: `entity:workDetail:${id}`,
    seq,
    entity: {
      collection: "workDetail",
      id,
      value: {
        id,
        rootId,
        title: id,
        status: "running",
        version: 1,
        created: 1,
        updated: 1,
      },
    },
  });
  const first = row("first", "lead", 1),
    second = row("second", "lead", 2),
    hidden = row("hidden", "other", 3);
  // WorkItem permits extra properties. A direct agent field never changes its root-only scope.
  Object.assign(hidden.entity.value, { agent: "lead" });
  const send = (values: unknown[]) =>
    emit(values.map((value) => ({ toJSON: () => structuredClone(value) })));
  send([first, second, hidden]);
  const initial = accept.mock.calls.at(-1)![0];
  expect(initial.items).toHaveLength(2);
  send([first, second, { ...hidden, seq: 4 }]);
  expect(accept).toHaveBeenCalledTimes(1);
  send([
    {
      ...first,
      seq: 5,
      entity: {
        ...first.entity,
        value: { ...first.entity.value, status: "review" },
      },
    },
    second,
    hidden,
  ]);
  expect(accept).toHaveBeenCalledTimes(2);
  expect(accept.mock.calls.at(-1)![0].items[1]).toBe(initial.items[1]);
  stop();
});

it("clears a retained detail after its authoritative local row is removed", async () => {
  let emit!: (rows: { toJSON: () => unknown }[]) => void;
  const unsubscribe = vi.fn();
  const database = vi.fn(async () => ({
    db: {
      projections: {
        find: () => ({
          $: {
            subscribe: ({ next }: { next: typeof emit }) => {
              emit = next;
              return { unsubscribe };
            },
          },
        }),
      },
    },
  }));
  const observe = createEntityDataObserver(
    database as unknown as ReturnType<typeof createSyncClient>["syncDatabase"],
  );
  const accept = vi.fn();
  const stop = observe(
    { kind: "agentDetail", agentId: "visible" },
    accept,
    vi.fn(),
  );
  await vi.waitFor(() => expect(emit).toBeTypeOf("function"));
  const value = { id: "visible", name: "Current detail" };
  emit([
    {
      toJSON: () => ({
        id: "entity:agentDetail:visible",
        entity: { collection: "agentDetail", id: "visible", value },
      }),
    },
  ]);
  expect(accept).toHaveBeenLastCalledWith(value);
  emit([]);
  expect(accept).toHaveBeenLastCalledWith(undefined);
  stop();
  expect(unsubscribe).toHaveBeenCalledOnce();
});

it("clears confirmed missing task detail through the real entity observer and facade", async () => {
  let emit!: (rows: { toJSON(): unknown }[]) => void;
  const observe = createEntityDataObserver((async () => ({
    db: {
      projections: {
        find: () => ({
          $: {
            subscribe: ({ next }: { next: typeof emit }) => {
              emit = next;
              return { unsubscribe() {} };
            },
          },
        }),
      },
    },
  })) as ReturnType<typeof createSyncClient>["syncDatabase"]);
  let changed!: (version?: ResourceVersion) => void;
  const subscriptions = createSubscriptions(
    {
      watchResourceChanges: (_ref, accept) => {
        changed = accept;
        return () => {};
      },
      watchResourceConnection: (accept) => {
        accept("live", "open", {});
        return () => {};
      },
    },
    observe,
  );
  const store = createServerStore(subscriptions);
  const slot = store.taskDetail(taskId("chosen"));
  const stop = autorun(() => slot.current);
  try {
    expect(slot.current.status).toBe("loading");
    await vi.waitFor(() => expect(emit).toBeTypeOf("function"));
    const row = (seq: number) => ({
      toJSON: () => ({
        id: "entity:taskDetail:chosen",
        seq,
        entity: {
          collection: "taskDetail",
          id: "chosen",
          value: {
            id: "chosen",
            agent: "lead",
            created: 1,
            kind: "tool",
            status: "completed",
            arguments: "private input",
          },
        },
      }),
    });
    changed({
      epoch: "test",
      revision: 1,
      cursor: { workspaceId: "test", seq: 1 },
      replyKind: "snapshot",
    });
    emit([row(1)]);
    await vi.waitFor(() => expect(slot.current.status).toBe("live"));
    const before = slot.current;
    expect(before.status === "live" && before.data.arguments).toBe(
      "private input",
    );
    emit([]);
    await vi.waitFor(() => expect(slot.current.status).toBe("absent"));
    expect("data" in slot.current).toBe(false);
    emit([row(3)]);
    await vi.waitFor(() => expect(slot.current.status).toBe("live"));
    const restored = slot.current;
    expect(restored.status === "live" && restored.data.arguments).toBe(
      "private input",
    );
    expect(
      restored.status === "live" && before.status === "live" && restored.data,
    ).not.toBe(before.status === "live" && before.data);
  } finally {
    stop();
    store.dispose();
    subscriptions.dispose();
  }
});

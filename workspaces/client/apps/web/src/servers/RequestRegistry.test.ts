// @vitest-environment happy-dom
import { autorun } from "mobx";
import { afterEach, expect, it, vi } from "vitest";
import { ApiError, NetworkTimeoutError } from "../api.ts";
import { z } from "zod";
import {
  RequestRegistry,
  requestOperation,
  type OperationContext,
  type RequestEntry,
  type ReceiptProof,
} from "./RequestRegistry.ts";

type Send = (entry: RequestEntry) => Promise<"applied" | void>;
type Watch = (
  entry: RequestEntry,
  observe: (proof: ReceiptProof) => void,
) => () => void;
const handlers = new Map<OperationContext, { send: Send; watch: Watch }>();
const handler = (context: OperationContext) => {
  const found = handlers.get(context);
  if (!found) throw new Error("Unknown test registry context.");
  return found;
};
const operation = requestOperation({
  path: "/api/rename",
  body: z.object({ id: z.string(), name: z.string() }).strict(),
  label: (body) => body.name,
  send: (context, _body, entry) => handler(context).send(entry),
  watch: (context, _body, entry, observe) =>
    handler(context).watch(entry, observe),
});
const intent = operation.intent(["workspace", "rename", "agent"], {
  id: "agent",
  name: "name",
});
const registries: RequestRegistry[] = [];
function registryWith(storage: Storage, send: Send, watch: Watch) {
  const context: OperationContext = {
    api: { post: vi.fn() },
    store: () => {
      throw new Error("No store in registry tests.");
    },
    notify: () => {},
  };
  handlers.set(context, { send, watch });
  const registry = new RequestRegistry(storage, context, [operation]);
  registries.push(registry);
  return registry;
}
function fixture(storage: Storage = window.localStorage) {
  const send = vi.fn<Send>(async () => {});
  const registry = registryWith(storage, send, () => () => {});
  return { registry, send };
}
const flush = async () => {
  await Promise.resolve();
  await Promise.resolve();
};
afterEach(() => {
  registries.splice(0).forEach((registry) => registry.dispose());
  localStorage.clear();
  vi.restoreAllMocks();
  vi.useRealTimers();
});
it("timeout then explicit retry reuses its frozen identity/body and ignores simultaneous retries", async () => {
  const { registry, send } = fixture();
  send.mockRejectedValueOnce(new NetworkTimeoutError());
  const body = { id: "agent", name: "name" };
  const entry = registry.start(operation.intent(intent.key, body));
  body.name = "edited";
  await flush();
  expect(entry.view.state).toBe("unknown");
  expect(entry.view.canRetry).toBe(true);
  const stop = autorun(() => entry.view);
  await Promise.all([
    registry.retry(entry.requestId),
    registry.retry(entry.requestId),
  ]);
  expect(send).toHaveBeenCalledTimes(2);
  expect(send.mock.calls[0]).toEqual(send.mock.calls[1]);
  expect(entry.body).toEqual(intent.body);
  expect(Object.isFrozen(entry.body)).toBe(true);
  stop();
});
it("distinct equal-text actions get new ids and late A cannot change B", async () => {
  const { registry, send } = fixture();
  let finish: (() => void) | undefined;
  send.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  send.mockRejectedValueOnce(new NetworkTimeoutError());
  const a = registry.start(intent),
    b = registry.start(intent);
  await flush();
  expect(a.requestId).not.toBe(b.requestId);
  finish?.();
  await flush();
  expect(registry.latest(intent.key)).toBe(b);
  expect(b.state).toBe("unknown");
});
it.each([false, true])(
  "SSE proof wins over HTTP loss (after timeout: %s)",
  async (afterTimeout) => {
    const { registry, send } = fixture();
    let reject: ((reason: Error) => void) | undefined;
    send.mockImplementationOnce(
      () =>
        new Promise<void>((_resolve, fail) => {
          reject = fail;
        }),
    );
    const entry = registry.start(intent);
    const stop = autorun(() => entry.view);
    if (afterTimeout) {
      reject?.(new NetworkTimeoutError());
      await flush();
    }
    registry.observe(entry.requestId, { kind: "applied" });
    reject?.(new NetworkTimeoutError());
    await flush();
    expect(entry.state).toBe("applied-from-SSE");
    expect(entry.view.canRetry).toBe(false);
    stop();
  },
);
it("server subtrees with identical targets have independent status", async () => {
  const a = fixture(localStorage),
    b = fixture(sessionStorage);
  a.send.mockRejectedValueOnce(new NetworkTimeoutError());
  const first = a.registry.start(intent),
    second = b.registry.start(intent);
  const stop = autorun(() => second.view);
  await flush();
  expect(first.state).toBe("unknown");
  expect(second.state).toBe("confirmed");
  expect(a.registry.get(second.requestId)).toBeUndefined();
  stop();
});
it("reload retains all unresolved identities, dismiss and 30 seconds never forget unknown", async () => {
  vi.useFakeTimers();
  const a = fixture();
  a.send.mockRejectedValue(new NetworkTimeoutError());
  const first = a.registry.start(intent),
    second = a.registry.start(intent);
  await flush();
  a.registry.dismiss(second.requestId);
  await vi.advanceTimersByTimeAsync(30_001);
  a.registry.dispose();
  const b = fixture();
  expect(b.send).not.toHaveBeenCalled();
  expect(
    b.registry.forTarget("workspace", "agent").map((e) => e.requestId),
  ).toEqual([first.requestId, second.requestId]);
  expect(b.registry.latest(intent.key)?.state).toBe("unknown");
  await b.registry.retry(first.requestId);
  expect(b.send).toHaveBeenCalledWith(
    expect.objectContaining({ requestId: first.requestId, body: intent.body }),
  );
});
it("storage failure prevents dispatch", () => {
  const { registry, send } = fixture();
  vi.spyOn(localStorage, "setItem").mockImplementationOnce(() => {
    throw new Error("full");
  });
  expect(() => registry.start(intent)).toThrow("full");
  expect(send).not.toHaveBeenCalled();
});
it("a later rejection cannot erase prior uncertainty, but a fresh rejection is failed", async () => {
  const { registry, send } = fixture();
  send
    .mockRejectedValueOnce(new NetworkTimeoutError())
    .mockRejectedValue(new ApiError("bad", 400));
  const entry = registry.start(intent);
  const stop = autorun(() => entry.view);
  await flush();
  await registry.retry(entry.requestId);
  expect(entry.state).toBe("unknown");
  const fresh = registry.start(intent);
  await flush();
  expect(fresh.state).toBe("failed");
  stop();
});
it("releasing terminal history stops its proof watch but observer loss keeps unknown", async () => {
  const stopProof = vi.fn();
  const registry = registryWith(
    localStorage,
    async () => {
      throw new NetworkTimeoutError();
    },
    () => stopProof,
  );
  const entry = registry.start(intent);
  const stop = autorun(() => entry.view);
  await flush();
  stop();
  expect(registry.get(entry.requestId)).toBe(entry);
  expect(stopProof).not.toHaveBeenCalled();
  const read = autorun(() => entry.view);
  registry.observe(entry.requestId, { kind: "applied" });
  expect(registry.get(entry.requestId)).toBe(entry);
  read();
  await flush();
  expect(registry.get(entry.requestId)).toBeUndefined();
  expect(stopProof).toHaveBeenCalledOnce();
});
it("reload reconciles every saved entry even when synchronous proof removes records", async () => {
  const a = fixture();
  a.send.mockRejectedValue(new NetworkTimeoutError());
  a.registry.start(intent);
  a.registry.start(intent);
  await flush();
  a.registry.dispose();
  const watch = vi.fn(
    (_entry: RequestEntry, observe: (proof: ReceiptProof) => void) => {
      observe({ kind: "applied" });
      return () => {};
    },
  );
  registryWith(localStorage, async () => {}, watch);
  expect(watch).toHaveBeenCalledTimes(2);
  expect(registries.at(-1)?.forTarget("workspace", "agent")).toHaveLength(0);
});
it("a closed scope ignores late HTTP and a replacement retains the interrupted identity", async () => {
  const a = fixture();
  let finish: (() => void) | undefined;
  a.send.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const entry = a.registry.start(intent);
  a.registry.dispose();
  const b = fixture();
  const restored = b.registry.get(entry.requestId);
  finish?.();
  await flush();
  expect(restored?.state).toBe("unknown");
  expect(b.send).not.toHaveBeenCalled();
});
it("an admitted nested body is normalized once and stays immutable for every attempt", async () => {
  const nested = requestOperation({
    path: "/api/rename",
    body: z.object({ id: z.string(), tags: z.array(z.string()) }),
    label: (body) => body.id,
    send: async (_context, body) => {
      expect(() => body.tags.push("changed")).toThrow();
    },
  });
  const value = { id: "agent", tags: ["a"], extra: true };
  const admitted = nested.intent(["workspace", "rename", "agent"], value);
  value.tags.push("caller edit");
  expect(admitted.body).toEqual({ id: "agent", tags: ["a"] });
  const { registry } = fixture();
  await registry.start(admitted).completion;
  expect(admitted.body).toEqual({ id: "agent", tags: ["a"] });
});
it("an unknown request of an unregistered operation stays visible and cannot resend", async () => {
  // Seed a journal from an older client before the registry restores it.
  localStorage.setItem(
    "studio-server-write:request:old",
    JSON.stringify({
      requestId: "old",
      key: ["workspace", "retired", "target"],
      path: "/api/retired",
      body: { id: "target" },
      state: "unknown",
    }),
  );
  const { registry, send } = fixture();
  const entry = registry.latest(["workspace", "retired", "target"]);
  expect(entry?.view).toMatchObject({ state: "unknown", canRetry: false });
  await registry.retry("old");
  expect(send).not.toHaveBeenCalled();
  expect(entry?.state).toBe("unknown");
});
it("operation bodies are non-transforming plain JSON so revival resends the admitted body", () => {
  const send = async () => {};
  requestOperation({
    path: "/api/rename",
    // @ts-expect-error A transforming schema would change the body on revival.
    body: z.object({ count: z.number().transform(String) }),
    label: () => "count",
    send,
  });
  requestOperation({
    path: "/api/rename",
    // @ts-expect-error Binary views are not JSON request bodies.
    body: z.object({ payload: z.instanceof(Uint8Array) }),
    label: () => "payload",
    send,
  });
});

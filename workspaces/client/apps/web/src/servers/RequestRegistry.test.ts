// @vitest-environment happy-dom
import { autorun } from "mobx";
import { afterEach, expect, it, vi } from "vitest";
import { ApiError, NetworkTimeoutError } from "../api.ts";
import {
  RequestRegistry,
  type Intent,
  type RequestEntry,
  type ReceiptProof,
} from "./RequestRegistry.ts";

const intent: Intent = {
  key: ["workspace", "rename", "agent"],
  path: "/api/rename",
  body: { id: "agent", name: "name" },
};
const registries: RequestRegistry[] = [];
function fixture(storage: Storage = window.localStorage) {
  const send = vi.fn(async () => {});
  const registry = new RequestRegistry(storage, send, () => () => {});
  registries.push(registry);
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
  const entry = registry.start({ ...intent, body });
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
  const registry = new RequestRegistry(
    localStorage,
    async () => {
      throw new NetworkTimeoutError();
    },
    () => stopProof,
  );
  registries.push(registry);
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
  const registry = new RequestRegistry(localStorage, async () => {}, watch);
  registries.push(registry);
  expect(watch).toHaveBeenCalledTimes(2);
  expect(registry.forTarget("workspace", "agent")).toHaveLength(0);
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

import { setStartupFixture } from "./components/observableFixture.tsx";
// @vitest-environment happy-dom
import { componentSnapshot as snapshotFixture } from "./components/componentTestFixtures.ts";
import { conversationActionsFixture } from "./components/observableFixture.tsx";
import { required } from "./components/componentTestFixtures.ts";
import { expect, it, vi } from "vitest";
import type { Agent } from "./types.ts";
function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    key: (index) => [...values.keys()][index] ?? null,
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => {
      values.set(key, value);
    },
    removeItem: (key) => {
      values.delete(key);
    },
    clear: () => values.clear(),
  };
}
it("retries an uncertain action with its original request and server after another owner is selected", async () => {
  const postA = vi.fn().mockRejectedValue(new Error("Unknown outcome"));
  const postB = vi.fn().mockRejectedValue(new Error("Unknown outcome"));
  const makeOwner = (post: typeof postA) => {
    const area = conversationActionsFixture();
    area.scope.services.api.post = post;
    Object.defineProperty(area.scope.services, "storage", {
      value: memoryStorage(),
    });
    setStartupFixture(area, {
      ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
      stateDir: "/same/path",
    });
    area.startup.workspaceId = "same-workspace";
    return area.native;
  };
  const a = makeOwner(postA),
    b = makeOwner(postB);
  const agent: Agent = {
    id: "same-chat",
    accountKey: "default",
    threadId: "same-thread",
    epoch: 0,
  };
  await expect(a.submit(agent, "compact")).rejects.toThrow("Unknown outcome");
  await expect(b.submit(agent, "review")).rejects.toThrow("Unknown outcome");
  const requestA = a.pending(agent.id)!;
  const requestB = b.pending(agent.id)!;
  expect(requestA.action).toBe("compact");
  expect(requestB.action).toBe("review");
  expect(requestA.request_id).not.toBe(requestB.request_id);
  await expect(a.submit(agent, "compact")).rejects.toThrow("Unknown outcome");

  expect(required(postA.mock.calls[1])[1]).toEqual(requestA);
  expect(postB).toHaveBeenCalledTimes(1);
  expect(b.pending(agent.id)).toEqual(requestB);
});

it("a late receipt settles its original workspace after disposal and preserves the newer saved action", async () => {
  const reply = Promise.withResolvers<unknown>();
  const post = vi
    .fn()
    .mockReturnValueOnce(reply.promise)
    .mockRejectedValue(new TypeError("reply lost"));
  const notify = vi.fn();
  const area = conversationActionsFixture();
  area.scope.services.api.post = post;
  Object.defineProperty(area.scope.services, "storage", {
    value: memoryStorage(),
  });
  setStartupFixture(area, {
    ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
    stateDir: "first",
  });
  area.startup.workspaceId = "first";
  area.notices.notify = notify;
  const owner = area.native;
  const stop = owner.start();
  const agent: Agent = {
    id: "same-chat",
    accountKey: "default",
    threadId: "thread",
    epoch: 0,
  };
  const first = owner.submit(agent, "compact");
  const request = required(owner.pending(agent.id));
  setStartupFixture(area, {
    ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
    stateDir: "second",
  });
  area.startup.workspaceId = "second";
  await expect(owner.submit(agent, "review")).rejects.toThrow("reply lost");
  const newer = owner.pending(agent.id);
  stop();
  reply.resolve({ receipt: { requestId: request.request_id }, replayed: true });
  await first;
  expect(owner.readKey(agent.id, "first:same-chat")).toBeNull();
  expect(owner.pending(agent.id)).toEqual(newer);
  expect(notify).not.toHaveBeenCalled();
  area.scope.services.close();
});

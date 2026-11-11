import { autorun, reaction } from "mobx";
import { afterEach, expect, it, vi } from "vitest";

vi.mock("../servers/net/api.ts", () => ({
  syncPost: vi.fn(),
  ApiError: class extends Error {},
}));
vi.mock(import("../unsent/store.ts"), async (importOriginal) => ({
  ...(await importOriginal()),
  draftStore: () => ({
    put: async () => {},
    delete: async () => {},
    loadAll: async () => [],
  }),
}));
vi.mock("./subscriptions/events.ts", () => ({ watchResourceChanges: vi.fn() }));
vi.mock("./resourceEvents.ts", () => ({ replaceStream: vi.fn() }));
import { createDrafts } from "./drafts.ts";
import type { ResourceVersion } from "./subscriptions/contracts.ts";

afterEach(() => {
  vi.useRealTimers();
});

it("reports an incomplete snapshot visibly while other drafts continue to sync", async () => {
  vi.useFakeTimers();
  let receive: ((version?: ResourceVersion) => void) | undefined;
  const post = vi.fn(async () => []);
  const drafts = createDrafts({
    api: { syncPost: post },
    resources: {
      watchResourceChanges: (_ref, listener) => {
        receive = listener;
        return () => {};
      },
      replaceStream: vi.fn(),
    },
  });
  const cleanup = drafts.start();
  receive?.({
    epoch: "e",
    revision: 3,
    reason: "initial",
    draftSnapshot: {
      complete: false,
      documents: [
        {
          id: "phone:good",
          seq: 1,
          _deleted: false,
          payload: JSON.stringify({
            id: "phone:good",
            device: "phone",
            session: "good",
            text: "kept",
            updated: 1,
          }),
        },
      ],
    },
  });
  expect(drafts.getDraft("good")).toBe("kept");
  drafts.setDrafts({ good: "edited" });
  await vi.advanceTimersByTimeAsync(3000);
  expect(post).toHaveBeenCalled();
  expect(drafts.error).toBe("Some saved drafts could not be loaded.");
  cleanup();
});

it("publishes an entire baseline before notifying either chat", () => {
  vi.useFakeTimers();
  let receive: ((version?: ResourceVersion) => void) | undefined;
  const stop = vi.fn();
  const drafts = createDrafts({
    api: { syncPost: vi.fn(async () => []) },
    resources: {
      watchResourceChanges: (_ref, listener) => {
        receive = listener;
        return stop;
      },
      replaceStream: vi.fn(),
    },
  });
  const a = vi.fn(() => expect(drafts.getDraft("b")).toBe("complete"));
  const b = vi.fn(() => expect(drafts.getDraft("a")).toBe("complete"));
  const stopA = reaction(() => drafts.getDraft("a"), a);
  const stopB = reaction(() => drafts.getDraft("b"), b);
  const cleanup = drafts.start();
  const row = (session: string) => ({
    id: `remote:${session}`,
    seq: 1,
    _deleted: false,
    payload: JSON.stringify({
      id: `remote:${session}`,
      device: "remote",
      session,
      text: "complete",
      updated: 1,
    }),
  });
  receive?.({
    epoch: "e",
    revision: 1,
    reason: "initial",
    draftSnapshot: { complete: true, documents: [row("a"), row("b")] },
  });
  expect(a).toHaveBeenCalledOnce();
  expect(b).toHaveBeenCalledOnce();
  stopA();
  stopB();
  cleanup();
  expect(stop).toHaveBeenCalledOnce();
});

it("reopens on a missing interval and keeps edits until the next baseline", async () => {
  vi.useFakeTimers();
  let receive: ((version?: ResourceVersion) => void) | undefined;
  const replaceStream = vi.fn();
  const post = vi.fn(async () => []);
  const drafts = createDrafts({
    api: { syncPost: post },
    resources: {
      watchResourceChanges: (_ref, listener) => {
        receive = listener;
        return () => {};
      },
      replaceStream,
    },
  });
  const cleanup = drafts.start();
  receive?.({
    epoch: "e",
    revision: 1,
    reason: "initial",
    draftSnapshot: { complete: true, documents: [] },
  });
  drafts.setDrafts({ chat: "typed" });
  receive?.({
    epoch: "e",
    revision: 3,
    reason: "change",
    draftChanges: { after: 2, through: 3, documents: [] },
  });
  expect(replaceStream).toHaveBeenCalledOnce();
  await vi.advanceTimersByTimeAsync(3000);
  expect(post).not.toHaveBeenCalled();
  expect(drafts.getDraft("chat")).toBe("typed");
  receive?.({
    epoch: "e",
    revision: 3,
    reason: "reconnect",
    draftSnapshot: { complete: true, documents: [] },
  });
  await vi.advanceTimersByTimeAsync(3000);
  expect(post).toHaveBeenCalledOnce();
  cleanup();
});

it("shows the shared notice when another unsent input cannot be saved", () => {
  const windowEvents = new EventTarget();
  vi.stubGlobal("window", windowEvents);
  const drafts = createDrafts({
    api: { syncPost: vi.fn(async () => []) },
    resources: { watchResourceChanges: () => () => {}, replaceStream: vi.fn() },
  });
  const cleanup = drafts.start();
  windowEvents.dispatchEvent(new Event("studio-unsent-input-failure"));
  expect(drafts.error).toBe(
    "Draft changes could not be saved. Keep this chat open.",
  );
  cleanup();
  vi.unstubAllGlobals();
});

it("tracks draft text directly without subscribing or bumping a revision", () => {
  vi.useFakeTimers();
  const drafts = createDrafts({
    api: { syncPost: vi.fn(async () => []) },
    resources: { watchResourceChanges: () => () => {}, replaceStream: vi.fn() },
  });
  const seen: string[] = [];
  const stop = autorun(() => seen.push(drafts.getDraft("chat")));
  drafts.setDrafts({ chat: "typed" });
  drafts.setDrafts({ chat: "typed", other: "unrelated" });
  expect(seen).toEqual(["", "typed"]);
  stop();
});

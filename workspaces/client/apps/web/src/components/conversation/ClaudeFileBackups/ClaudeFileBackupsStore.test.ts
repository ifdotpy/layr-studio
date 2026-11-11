import { autorun } from "mobx";
import * as api from "../../../servers/net/api.ts";
import { afterEach, expect, it, vi } from "vitest";
import type { PostResult } from "../../../api.ts";
import { ClaudeFileBackupsStore } from "./ClaudeFileBackupsStore.ts";

const usage = {
  count: 2,
  maxCount: 8,
  bytes: 12,
  maxBytes: 128,
  released: 0,
} satisfies PostResult<"/api/claude/file-backups">;
const settle = async () => {
  for (let i = 0; i < 8; i++) await Promise.resolve();
};

it("loads only when observed and retains the exact release identity after an uncertain response", async () => {
  const post = vi.spyOn(api, "post").mockResolvedValue(usage);
  const store = new ClaudeFileBackupsStore(api, "agent");
  expect(post).not.toHaveBeenCalled();
  const stop = autorun(() => store.usage);
  await settle();
  expect(store.usage).toEqual(usage);
  expect(post).toHaveBeenCalledTimes(1);
  const released = vi.fn();
  post.mockRejectedValueOnce(new Error("response lost"));
  await store.release(released);
  const first = post.mock.calls[1]?.[1];
  expect(store.message).toContain("response lost");
  post.mockResolvedValueOnce({ ...usage, released: 2 });
  await store.release(released);
  expect(post.mock.calls[2]?.[1]).toEqual(first);
  expect(released).toHaveBeenCalledOnce();
  expect(store.requestId).toBeNull();
  stop();
  store.dispose();
});

it("does not publish a pending initial read after its owner disposes", async () => {
  const pending = Promise.withResolvers<typeof usage>();
  vi.spyOn(api, "post").mockReturnValue(pending.promise);
  const store = new ClaudeFileBackupsStore(api, "agent");
  const stop = autorun(() => store.usage);
  store.dispose();
  pending.resolve(usage);
  await settle();
  expect(store.usage).toBeNull();
  stop();
});

afterEach(() => vi.restoreAllMocks());

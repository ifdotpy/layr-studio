import { expect, it, vi } from "vitest";
import { createModelReader } from "./modelReader.ts";
import type { GetResult } from "../api";

const owner = {
  id: "remote",
  origin: "https://remote.test",
  credentialId: "first",
};
const models: GetResult<"/api/models"> = { data: [] };

it("coalesces only simultaneous queries within the exact credential owner", async () => {
  let finish: ((result: typeof models) => void) | undefined;
  const read = vi.fn(
    () =>
      new Promise<typeof models>((resolve) => {
        finish = resolve;
      }),
  );
  const reader = createModelReader(owner, read);
  const first = reader.read({ query: { account_key: "same" } });
  const second = reader.read({ query: { account_key: "same" } });
  expect(read).toHaveBeenCalledTimes(1);
  finish?.(models);
  expect(await first).toBe(models);
  expect(await second).toBe(models);
  const next = reader.read({ query: { account_key: "same" } });
  expect(read).toHaveBeenCalledTimes(2);
  finish?.(models);
  await next;
  const replacement = createModelReader(
    { ...owner, credentialId: "second" },
    read,
  );
  expect(replacement.scope).not.toBe(reader.scope);
  const other = replacement.read({ query: { account_key: "same" } });
  expect(read).toHaveBeenCalledTimes(3);
  finish?.(models);
  await other;
});

it("aborts one wait without cancelling the shared request or another waiter", async () => {
  let finish: ((result: typeof models) => void) | undefined;
  const read = vi.fn(
    () =>
      new Promise<typeof models>((resolve) => {
        finish = resolve;
      }),
  );
  const reader = createModelReader(owner, read);
  const controller = new AbortController();
  const first = reader.read({ signal: controller.signal });
  const rejected = expect(first).rejects.toMatchObject({ name: "AbortError" });
  const second = reader.read({});
  controller.abort();
  await rejected;
  expect(read).toHaveBeenCalledExactlyOnceWith({ query: undefined });
  finish?.(models);
  expect(await second).toBe(models);
});

it("clears rejected reads and skips requests for already cancelled callers", async () => {
  const read = vi.fn(async () => models);
  const reader = createModelReader(owner, read);
  await expect(
    reader.read({ signal: AbortSignal.abort() }),
  ).rejects.toMatchObject({ name: "AbortError" });
  expect(read).not.toHaveBeenCalled();
  read.mockRejectedValueOnce(new Error("offline"));
  await expect(reader.read({})).rejects.toThrow("offline");
  expect(await reader.read({})).toBe(models);
  expect(read).toHaveBeenCalledTimes(2);
});

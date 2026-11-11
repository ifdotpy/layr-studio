import { flowResult } from "mobx";
import { expect, it, vi } from "vitest";
import { AttachmentSave } from "./AttachmentSave.ts";

it("captures the destination before picking and retries the original files", async () => {
  const store = new AttachmentSave();
  const picked = Promise.withResolvers<File[]>();
  const files = [new File(["a"], "a.txt")];
  const add = vi
    .fn<(files: File[]) => Promise<void>>()
    .mockRejectedValueOnce(new Error("save failed"))
    .mockResolvedValue(undefined);
  const notify = vi.fn(),
    reset = vi.fn();
  const saving = flowResult(
    store.save(() => picked.promise, add, notify, reset),
  );
  picked.resolve(files);
  await saving;
  expect(store.pending?.files).toBe(files);
  expect(notify).toHaveBeenCalledWith("save failed");
  store.retry(notify, reset);
  await vi.waitFor(() => expect(reset).toHaveBeenCalledOnce());
  expect(add.mock.calls.map(([value]) => value)).toEqual([files, files]);
  expect(store.pending).toBeNull();
  expect(store.error).toBe("");
});

it("keeps a newer failed upload through picker cancellation and an older completion", async () => {
  const store = new AttachmentSave();
  const older = Promise.withResolvers<void>();
  const notify = vi.fn(),
    reset = vi.fn();
  const first = flowResult(
    store.save([new File(["a"], "a.txt")], () => older.promise, notify, reset),
  );
  const files = [new File(["b"], "b.txt")];
  await flowResult(
    store.save(
      files,
      async () => {
        throw new Error("newer failed");
      },
      notify,
      reset,
    ),
  );
  const pending = store.pending;
  await flowResult(store.save(async () => [], vi.fn(), notify, reset));
  older.resolve();
  await first;
  expect(store.pending).toBe(pending);
  expect(store.pending?.files).toBe(files);
  expect(store.error).toBe("newer failed");
  expect(reset).not.toHaveBeenCalled();
});

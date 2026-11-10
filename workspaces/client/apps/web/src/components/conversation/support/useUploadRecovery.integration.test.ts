import { required } from "../ComposerAttachments/componentTestFixtures.ts";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  effects: [] as Array<() => void | (() => void)>,
  uploadListeners: new Set<() => void>(),
  resumeListeners: new Set<() => void>(),
  pendingUploads: vi.fn(),
  deliverUpload: vi.fn(),
  finishUpload: vi.fn(),
  removeUpload: vi.fn(),
  uploadCancelled: vi.fn(),
}));

vi.mock("react", () => ({
  useMemo: (create: () => unknown) => create(),
  useEffect: (effect: () => void | (() => void)) => {
    mocks.effects.push(effect);
  },
  useRef: (initial: unknown) => ({ current: initial }),
  useState: (initial: unknown) => [initial, vi.fn()],
}));
vi.mock("../../../sync/uploads.ts", () => ({
  pendingUploads: mocks.pendingUploads,
  deliverUpload: mocks.deliverUpload,
  finishUpload: mocks.finishUpload,
  removeUpload: mocks.removeUpload,
  uploadCancelled: mocks.uploadCancelled,
  onUploadsChanged: (listener: () => void) => {
    mocks.uploadListeners.add(listener);
    return () => mocks.uploadListeners.delete(listener);
  },
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerUploads: () => ({
      pendingUploads: mocks.pendingUploads,
      deliverUpload: mocks.deliverUpload,
      finishUpload: mocks.finishUpload,
      removeUpload: mocks.removeUpload,
      uploadCancelled: mocks.uploadCancelled,
      onUploadsChanged: (listener: () => void) => {
        mocks.uploadListeners.add(listener);
        return () => mocks.uploadListeners.delete(listener);
      },
    }),
  }),
);
vi.mock("../../../sync/resume.ts", () => ({
  onResume: (listener: () => void) => {
    mocks.resumeListeners.add(listener);
    return () => mocks.resumeListeners.delete(listener);
  },
}));
vi.mock("../../../servers/net/api.ts", () => ({
  errorText: (error: unknown) =>
    error instanceof Error ? error.message : String(error),
}));
vi.mock("../../../sync/readRetry.ts", () => ({
  retryableReadError: (error: unknown) => error instanceof TypeError,
}));

import { useUploadRecovery } from "./useUploadRecovery.ts";
import type { PendingUpload } from "../../../sync/uploads.ts";

function mountOwner(
  attach: Parameters<typeof useUploadRecovery>[2] = async () => true,
) {
  const store = useUploadRecovery("/workspace/state", "workspace-1", attach);
  const cleanups = mocks.effects.splice(0).flatMap((effect) => {
    const cleanup = effect();
    return cleanup ? [cleanup] : [];
  });
  return { store, unmount: () => cleanups.forEach((cleanup) => cleanup()) };
}

function mount() {
  return mountOwner().unmount;
}

function upload(): PendingUpload {
  return {
    id: "upload-stable-id",
    workspace: "workspace-1",
    stateDir: "/workspace/state",
    agent: "lead",
    name: "notes.txt",
    size: 12,
    mime: "text/plain",
    created: 1,
  };
}

async function flushMicrotasks() {
  for (let attempt = 0; attempt < 30; attempt++) await Promise.resolve();
}

beforeEach(() => {
  vi.useFakeTimers();
  mocks.effects.length = 0;
  mocks.uploadListeners.clear();
  mocks.resumeListeners.clear();
  mocks.pendingUploads.mockReset();
  mocks.deliverUpload.mockReset();
  mocks.finishUpload.mockReset().mockResolvedValue(undefined);
  mocks.removeUpload.mockReset();
  mocks.uploadCancelled.mockReset().mockReturnValue(false);
  vi.stubGlobal("navigator", { onLine: true });
  vi.stubGlobal("window", new EventTarget());
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("useUploadRecovery", () => {
  it("does not poll an empty queue and stops reads after unmount", async () => {
    mocks.pendingUploads.mockResolvedValue([]);
    const unmount = mount();
    await flushMicrotasks();
    expect(mocks.pendingUploads).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(180_000);
    await flushMicrotasks();
    expect(mocks.pendingUploads).toHaveBeenCalledTimes(1);

    unmount();
    expect(mocks.uploadListeners.size).toBe(0);
    expect(mocks.resumeListeners.size).toBe(0);
    await vi.advanceTimersByTimeAsync(180_000);
    await flushMicrotasks();
    expect(mocks.pendingUploads).toHaveBeenCalledTimes(1);
  });

  it("scans the captured workspace when a cross-tab upload change arrives", async () => {
    const row = upload();
    mocks.pendingUploads.mockResolvedValueOnce([]).mockResolvedValue([row]);
    mocks.deliverUpload.mockResolvedValue({ id: row.id });
    const unmount = mount();
    await flushMicrotasks();
    expect(mocks.pendingUploads).toHaveBeenCalledTimes(1);
    const changedInAnotherTab = required([...mocks.uploadListeners][0]);

    changedInAnotherTab();
    await flushMicrotasks();

    expect(mocks.pendingUploads.mock.calls).toEqual([
      ["/workspace/state", "workspace-1"],
      ["/workspace/state", "workspace-1"],
      ["/workspace/state", "workspace-1"],
    ]);
    expect(mocks.deliverUpload).toHaveBeenCalledWith(row);
    expect(mocks.finishUpload).toHaveBeenCalledWith(row.id);
    unmount();
  });

  it("retries the same upload after a transient IndexedDB outage longer than two minutes", async () => {
    const row = upload();
    mocks.pendingUploads.mockResolvedValue([row]);
    mocks.deliverUpload
      .mockRejectedValueOnce(new DOMException("Aborted", "AbortError"))
      .mockRejectedValueOnce(new DOMException("Aborted", "AbortError"))
      .mockRejectedValueOnce(new DOMException("Aborted", "AbortError"))
      .mockRejectedValueOnce(new DOMException("Aborted", "AbortError"))
      .mockRejectedValueOnce(new DOMException("Aborted", "AbortError"))
      .mockRejectedValueOnce(new DOMException("Aborted", "AbortError"))
      .mockRejectedValueOnce(new DOMException("Aborted", "AbortError"))
      .mockRejectedValueOnce(new DOMException("Aborted", "AbortError"))
      .mockResolvedValue({ id: row.id });
    const unmount = mount();
    await flushMicrotasks();
    expect(mocks.deliverUpload).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(180_000);
    await flushMicrotasks();

    expect(mocks.deliverUpload).toHaveBeenCalledTimes(9);
    expect(mocks.deliverUpload.mock.calls.map(([item]) => item.id)).toEqual(
      Array(9).fill(row.id),
    );
    expect(mocks.finishUpload).toHaveBeenCalledWith(row.id);
    expect(vi.getTimerCount()).toBe(0);
    unmount();
  });
});

it("keeps permanent delivery failures pending until an explicit retry", async () => {
  const row = upload(),
    asset = {
      id: row.id,
      image: false,
      mime: row.mime,
      name: row.name,
      size: row.size,
    };
  let rows = [row];
  mocks.pendingUploads.mockImplementation(async () => rows);
  mocks.deliverUpload
    .mockRejectedValueOnce(new Error("invalid file"))
    .mockResolvedValueOnce(asset);
  mocks.finishUpload.mockImplementation(async () => {
    rows = [];
  });
  const attach = vi.fn(async () => true),
    { store, unmount } = mountOwner(attach);
  try {
    await flushMicrotasks();
    expect(store.pending).toEqual([row]);
    expect(store.error).toContain("invalid file");
    expect(store.busy).toBe(false);
    for (const listener of mocks.uploadListeners) listener();
    await flushMicrotasks();
    expect(mocks.deliverUpload).toHaveBeenCalledTimes(1);
    store.retry();
    await flushMicrotasks();
    expect(mocks.deliverUpload).toHaveBeenCalledTimes(2);
    expect(mocks.deliverUpload.mock.calls[1]?.[0]).toEqual(row);
    expect(attach).toHaveBeenCalledTimes(1);
    expect(store.pending).toEqual([]);
    expect(store.error).toBe("");
  } finally {
    unmount();
  }
});
it("cancels a delivery on unmount without attaching or finishing its late result", async () => {
  const row = upload(),
    pending = Promise.withResolvers<{
      id: string;
      image: boolean;
      mime: string;
      name: string;
      size: number;
    }>();
  mocks.pendingUploads.mockResolvedValue([row]);
  mocks.deliverUpload.mockReturnValueOnce(pending.promise);
  const attach = vi.fn(async () => true),
    { store, unmount } = mountOwner(attach);
  await flushMicrotasks();
  expect(store.busy).toBe(true);
  unmount();
  pending.resolve({
    id: row.id,
    image: false,
    mime: row.mime,
    name: row.name,
    size: row.size,
  });
  await flushMicrotasks();
  expect(attach).not.toHaveBeenCalled();
  expect(mocks.finishUpload).not.toHaveBeenCalled();
  expect(mocks.pendingUploads).toHaveBeenCalledTimes(1);
});
it("automatically retries the identical IndexedDB row after a transaction abort", async () => {
  const row = upload(),
    asset = {
      id: row.id,
      image: false,
      mime: row.mime,
      name: row.name,
      size: row.size,
    };
  let rows = [row];
  mocks.pendingUploads.mockImplementation(async () => rows);
  mocks.deliverUpload
    .mockRejectedValueOnce(
      new DOMException("transaction aborted", "AbortError"),
    )
    .mockResolvedValueOnce(asset);
  mocks.finishUpload.mockImplementation(async () => {
    rows = [];
  });
  const { store, unmount } = mountOwner();
  try {
    await flushMicrotasks();
    expect(store.error).toContain("transaction aborted");
    await vi.advanceTimersByTimeAsync(1000);
    await flushMicrotasks();
    expect(mocks.deliverUpload.mock.calls.map((call) => call[0]?.id)).toEqual([
      row.id,
      row.id,
    ]);
    expect(store.error).toBe("");
    expect(store.pending).toEqual([]);
  } finally {
    unmount();
  }
});

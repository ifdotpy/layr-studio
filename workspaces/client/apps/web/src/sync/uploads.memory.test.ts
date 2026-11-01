import { afterEach, expect, it, vi } from "vitest";
import { createUploads, type UploadOptions } from "./uploads";
import type { StoredUpload } from "../unsent/store.ts";

afterEach(() => vi.unstubAllGlobals());

it("restores queued files after reload", async () => {
  const records = new Map<string, StoredUpload>();
  const buffer = {
    put: async (key: string, value: StoredUpload) => {
      records.set(key, value);
    },
    delete: async (key: string) => {
      records.delete(key);
    },
    loadAll: async (): Promise<[string, StoredUpload][]> => [...records],
  };
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal("BroadcastChannel", undefined);
  const options: UploadOptions = {
    databaseName: "test",
    buffer,
    syncDatabase: async () => ({ workspaceId: "workspace" }),
    syncGet: async () => ({ workspaceId: "workspace" }),
    refreshSession: vi.fn(),
    syncPost: vi.fn(),
  };
  const first = createUploads(options);
  await first.queueUploads(
    "state",
    "agent",
    [new File(["a"], "a.txt")],
    "workspace",
  );
  expect(await first.pendingUploads("state")).toHaveLength(1);
  expect(await createUploads(options).pendingUploads("state")).toHaveLength(1);
});

it("does not restore an upload removed while its request is in flight", async () => {
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal("BroadcastChannel", undefined);
  vi.stubGlobal(
    "FileReader",
    class {
      result = "data:text/plain;base64,YQ==";
      onload?: () => void;
      readAsDataURL() {
        this.onload?.();
      }
    },
  );
  type Asset = Awaited<ReturnType<UploadOptions["syncPost"]>>;
  let resolve!: (asset: Asset) => void;
  const response = new Promise<Asset>((done) => {
    resolve = done;
  });
  const post = vi.fn(() => response);
  const buffer = {
    put: async () => {},
    delete: async () => {},
    loadAll: async (): Promise<[string, StoredUpload][]> => [],
  };
  const options: UploadOptions = {
    databaseName: "test",
    buffer,
    syncDatabase: async () => ({ workspaceId: "workspace" }),
    syncGet: async () => ({ workspaceId: "workspace" }),
    refreshSession: vi.fn(async () => ({ token: "session" })),
    syncPost: post,
  };
  const uploads = createUploads(options);
  const [row] = await uploads.queueUploads(
    "state",
    "agent",
    [new File(["a"], "a.txt")],
    "workspace",
  );
  const delivery = uploads.deliverUpload(row!);
  await vi.waitFor(() => expect(post).toHaveBeenCalled());
  await uploads.removeUpload(row!.id);
  resolve({
    agent: "agent",
    created: 0,
    hash: "hash",
    id: row!.id,
    image: false,
    mime: "text/plain",
    name: row!.name,
    size: 1,
  });
  expect(await delivery).toBeNull();
  expect(await uploads.pendingUploads("state")).toEqual([]);
});

it("keeps the upload until the server confirms it", async () => {
  const { ApiError } = await import("../api.ts");
  const rows = new Map<string, StoredUpload>();
  const buffer = {
    put: async (key: string, value: StoredUpload) => {
      rows.set(key, value);
    },
    delete: async (key: string) => {
      rows.delete(key);
    },
    loadAll: async (): Promise<[string, StoredUpload][]> => [...rows],
  };
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal("BroadcastChannel", undefined);
  vi.stubGlobal(
    "FileReader",
    class {
      result = "data:text/plain;base64,YQ==";
      onload?: () => void;
      readAsDataURL() {
        this.onload?.();
      }
    },
  );
  const uploads = createUploads({
    databaseName: "test",
    buffer,
    syncDatabase: async () => ({ workspaceId: "workspace" }),
    syncGet: async () => ({ workspaceId: "workspace" }),
    refreshSession: vi.fn(async () => ({ token: "session" })),
    syncPost: vi.fn().mockRejectedValue(new ApiError("refused", 403)),
  });
  const [row] = await uploads.queueUploads(
    "state",
    "agent",
    [new File(["a"], "a.txt")],
    "workspace",
  );
  await expect(uploads.deliverUpload(row!)).rejects.toThrow("refused");
  expect(rows.size).toBe(1);
});

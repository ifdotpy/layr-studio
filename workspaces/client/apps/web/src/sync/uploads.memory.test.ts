import { afterEach, expect, it, vi } from "vitest";
import { createUploads, type UploadOptions } from "./uploads";

afterEach(() => vi.unstubAllGlobals());

it("restores queued files after reload", async () => {
  const records = new Map<string, string | Blob>();
  const buffer = {
    put: async (key: string, value: string | Blob) => {
      records.set(key, value);
    },
    delete: async (key: string) => {
      records.delete(key);
    },
    loadAll: async (): Promise<[string, string | Blob][]> => [...records],
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
    loadAll: async (): Promise<[string, string | Blob][]> => [],
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

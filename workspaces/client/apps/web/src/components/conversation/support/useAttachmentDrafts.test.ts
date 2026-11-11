import { afterEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  saved: vi.fn(() => ({ old: [] })),
  update: vi.fn(),
  ref: undefined as { current: unknown } | undefined,
  attachmentCancelled: (_id: string) => false as boolean,
}));
vi.mock("react", () => ({
  useRef: (current: unknown) => (mocks.ref ??= { current }),
  useState: () => [0, vi.fn()],
  useEffect: () => {},
  useLayoutEffect: () => {},
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerApi: () => ({ saved: mocks.saved }),
    useServerUploads: () => ({
      attachmentCancelled: mocks.attachmentCancelled,
    }),
    useServerLocalDrafts: () => ({ updateLocalDraft: mocks.update }),
    useServerStorageEventKey: () => () => undefined,
  }),
);
import { AttachmentDrafts } from "./useAttachmentDrafts.ts";
import { createUploads } from "../../../sync/uploads.ts";

afterEach(() => {
  mocks.ref = undefined;
  mocks.attachmentCancelled = () => false;
  vi.unstubAllGlobals();
});

it("keeps attachment selections in memory", async () => {
  const store = new AttachmentDrafts(mocks.attachmentCancelled, vi.fn());
  const { current: initial, set } = store;
  expect(initial).toEqual({});
  expect(await set({ chat: [] })).toBe(true);
  expect(mocks.saved).not.toHaveBeenCalled();
  expect(mocks.update).not.toHaveBeenCalled();
});

it("retains a confirmed upload for the send after finishing it", async () => {
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal("BroadcastChannel", undefined);
  const uploads = createUploads({
    databaseName: "test",
    buffer: {
      put: async () => {},
      delete: async () => {},
      loadAll: async () => [],
    },
    syncDatabase: async () => ({ workspaceId: "workspace" }),
    syncGet: async () => ({ workspaceId: "workspace" }),
    refreshSession: vi.fn(),
    syncPost: vi.fn(),
  });
  mocks.attachmentCancelled = uploads.attachmentCancelled;
  const asset = {
    agent: "agent",
    created: 0,
    hash: "hash",
    id: "upload-id",
    image: false,
    mime: "text/plain",
    name: "notes.txt",
    size: 1,
  };
  const store = new AttachmentDrafts(mocks.attachmentCancelled, vi.fn());
  const { set } = store;
  await set({ agent: [asset] });
  await uploads.finishUpload(asset.id);

  const attachments = store.current;
  expect(attachments.agent?.map((item) => item.id)).toEqual([asset.id]);
});

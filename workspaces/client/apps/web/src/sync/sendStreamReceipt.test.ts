import type { deliver as Deliver } from "./send";
import { afterEach, expect, it, vi } from "vitest";
import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
} from "../generated/apiSchema";

const { syncDatabase } = vi.hoisted(() => ({ syncDatabase: vi.fn() }));
vi.mock("./client", async (load) => ({
  ...(await load<typeof import("./client")>()),
  syncDatabase,
}));

afterEach(() => {
  vi.resetModules();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  syncDatabase.mockReset();
});

// A resend of a message whose receipt row exists on the server gets that stream row
// before the receipt of the answer. The message must still settle, never stay queued.
it("settles a resent message when the answer lists the stream receipt row first", async () => {
  vi.stubGlobal("navigator", { onLine: true });
  vi.stubGlobal("location", { origin: "http://studio.test" });
  vi.stubGlobal("window", {
    addEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  });
  vi.stubGlobal("document", {
    documentElement: { dataset: {} },
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  });
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() });
  syncDatabase.mockResolvedValue({ workspaceId: "workspace-a" });
  const headers = { [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH };
  vi.stubGlobal("fetch", async (request: Request) => {
    const url = new URL(request.url);
    if (url.pathname === "/api/sync/identity")
      return Response.json({ workspaceId: "workspace-a" }, { headers });
    if (url.pathname === "/api/session")
      return Response.json({ token: "token" }, { headers });
    if (url.pathname === "/api/commands")
      return Response.json(
        {
          status: "applied",
          ref_kind: "message",
          ref_id: "stable-id",
          rev: 1,
          seq: 9,
          rows: [
            {
              collection: "messageReceipt",
              id: "stable-id",
              seq: 9,
              payload: {
                collection: "messageReceipt",
                id: "stable-id",
                value: { id: "stable-id", agent: "chat", status: "delivered" },
              },
            },
            {
              collection: "messageReceipt",
              id: "stable-id",
              seq: 0,
              payload: { id: "stable-id", status: "delivered" },
            },
          ],
        },
        { headers },
      );
    throw new Error(`Unexpected request ${url.pathname}`);
  });
  const record = {
    id: "stable-id",
    payload: JSON.stringify({
      body: { id: "stable-id", room: "chat", text: "hello", delivery: "queue" },
      status: "queued",
      attempted: true,
      created: 1,
    }),
  };
  // Partial document fixture: delivery only reads id/getLatest/incrementalModify.
  const partialDoc = {
    id: "stable-id",
    getLatest: () => record,
    incrementalModify: async (
      change: (value: typeof record) => typeof record,
    ) => {
      Object.assign(record, change({ ...record }));
      return record;
    },
  };
  const doc = partialDoc as typeof partialDoc & Parameters<typeof Deliver>[0];
  const send = await import("./send");
  expect(await send.deliver(doc)).toMatchObject({ status: "delivered" });
  expect(JSON.parse(record.payload)).toMatchObject({
    status: "accepted",
    receipt: { id: "stable-id", status: "delivered" },
  });
});

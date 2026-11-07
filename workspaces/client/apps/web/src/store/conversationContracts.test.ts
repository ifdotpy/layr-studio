import { autorun } from "mobx";
import { expect, it, vi } from "vitest";
import {
  createComplaintMirror,
  createLiveDeltaMirror,
  createRoomMirror,
  createStateMirror,
  createTextBlobMirror,
  createTranscriptItemMirror,
  createTurnErrorMirror,
  serializeStateCollections,
  roomId,
  roomLegacy,
  complaintId,
} from "../generated/observableResources.ts";
import { RadioStatus } from "../generated/api.ts";
import type { components } from "../generated/api.ts";
import { createServerStore } from "./server.ts";
import { createSubscriptions } from "../sync/subscriptions/module.ts";
import type { ResourceRef } from "../sync/subscriptions/contracts.ts";

const radio: components["schemas"]["RoomRadio"] = {
  active: null,
  controls: {
    busy: false,
    inputBlocked: false,
    canPass: false,
    stopping: false,
    speaking: false,
    busyNotice: "old presentation",
  },
  error: null,
  next: [],
  revision: 1,
  speaker: null,
  status: RadioStatus.idle,
  teamId: "team",
};
const message = {
  id: "m",
  room: "r",
  sender: "a",
  senderName: "A",
  seq: 1,
  created: 1,
  text: "before",
  deliveries: {},
};
function legacyItems(mirror: ReturnType<typeof createRoomMirror>) {
  const current = mirror.current;
  if ("messages" in current) throw new Error("legacy array expected");
  return current;
}
const page = {
  messages: [message],
  room: { id: "r", radio },
  nextBefore: 2,
  nextAfter: null,
};

it("tracks room facts and legacy items with isolated mirror ownership", () => {
  const a = createRoomMirror(page);
  const b = createRoomMirror(page);
  if (Array.isArray(a.current) || !("messages" in a.current))
    throw new Error("room page");
  const row = a.current.messages[0]!;
  const room = a.current.room;
  const activeRadio = room.radio!;
  let reads = 0;
  const stop = autorun(() => {
    void row.text;
    reads++;
  });
  a.apply({
    ...page,
    messages: [{ ...message, text: "after" }],
    room: { id: "r", radio: { ...radio, revision: 2, speaker: "a" } },
  });
  expect(row.text).toBe("after");
  expect(reads).toBe(2);
  expect(room.radio).toBe(activeRadio);
  expect(activeRadio.revision).toBe(2);
  expect(activeRadio.controls).not.toHaveProperty("busyNotice");
  expect(b.current).toMatchObject({ messages: [{ text: "before" }] });
  a.apply([
    {
      id: "legacy",
      at: 1,
      author: "a",
      room: "r",
      text: "old",
      deliveries: {},
    },
  ]);
  const legacy = legacyItems(a)[0]!;
  a.apply([{ ...legacy, text: "new" }]);
  expect(legacyItems(a)[0]).toBe(legacy);
  expect(legacy.text).toBe("new");
  a.apply(page);
  expect(a.current).toMatchObject({ nextBefore: 2, nextAfter: null });
  stop();
});

it("round trips selected native collections and nested queues without presentation fields", () => {
  const a = createStateMirror();
  const entities = [
    {
      collection: "room" as const,
      id: "r",
      value: { id: "r", radio, members: ["a"] },
    },
    {
      collection: "complaint" as const,
      id: "c",
      value: { id: "c", version: 1 },
    },
    {
      collection: "messageReceipt" as const,
      id: "receipt",
      value: { id: "receipt", status: "queued" as const },
    },
    {
      collection: "queue" as const,
      id: "a",
      value: {
        revision: "1",
        capabilities: { reorder: true, receipts: true },
        items: [
          {
            id: "q",
            created: 1,
            text: "queued",
            kind: "user" as const,
            status: "queued" as const,
          },
        ],
      },
    },
  ] satisfies components["schemas"]["SyncEntityPayload"][];
  a.apply(entities);
  const queue = a.current.queues.get("a")!;
  const queued = queue.items[0]!;
  a.apply(
    entities.map((entity) =>
      entity.collection === "queue"
        ? {
            ...entity,
            value: {
              ...entity.value,
              revision: "2",
              items: [{ ...entity.value.items[0]!, text: "changed" }],
            },
          }
        : entity,
    ),
  );
  expect(queue.items[0]).toBe(queued);
  expect(queued.text).toBe("changed");
  const b = createStateMirror();
  b.apply(serializeStateCollections(a.current));
  expect(b.current.rooms.get("r")!.radio!.controls).not.toHaveProperty(
    "busyNotice",
  );
  expect(b.current.queues.get("a")!.revision).toBe("2");
  expect(b.current.complaints.has("c")).toBe(true);
  expect(b.current.messageReceipts.has("receipt")).toBe(true);
});

it("reconciles complaint responses and exposes only on-demand item/blob/error/delta models", () => {
  const response = {
    id: "reply",
    author: "a",
    at: 1,
    status: "resolved" as const,
    text: "first",
  };
  const dto = { id: "c", text: "question", version: 1, responses: [response] };
  const complaint = createComplaintMirror(dto);
  const reply = complaint.current.responses[0]!;
  complaint.apply({
    ...dto,
    version: 2,
    responses: [{ ...response, text: "second" }],
  });
  expect(complaint.current.responses[0]).toBe(reply);
  expect(reply.text).toBe("second");
  const item = createTranscriptItemMirror({
    id: "item",
    sourceId: "source",
    role: "assistant",
    text: "🙂",
  });
  expect(item.current.sourceId).toBe("source");
  const blob = createTextBlobMirror({ hash: "hash", size: 4, text: "🙂" });
  expect(blob.current.text).toBe("🙂");
  const turn = createTurnErrorMirror({
    agentId: "a",
    threadId: "thread",
    turnId: "turn",
    status: "failed",
    error: "failure",
  });
  expect(turn.current.error).toBe("failure");
  const delta = createLiveDeltaMirror({
    agentId: "a",
    itemId: "item",
    offset: 1,
    text: "𐐀x",
  });
  delta.apply({ agentId: "a", itemId: "item", offset: 3, text: "next" });
  expect(delta.current).toMatchObject({ offset: 3, text: "next" });
});

it("acquires room and complaint once per owner and releases while the store stays alive", async () => {
  vi.useFakeTimers();
  const release = vi.fn();
  const watch = vi.fn((_ref: ResourceRef) => release);
  const subscriptions = createSubscriptions({
    watchResourceChanges: watch,
    watchResourceConnection: (accept) => {
      accept("live", "open", {});
      return () => {};
    },
  });
  const store = createServerStore(subscriptions);
  const room = store.room(roomId("r"));
  expect(store.room(roomId("r"))).toBe(room);
  expect(store.room(roomId("r"), roomLegacy(true))).not.toBe(room);
  const complaint = store.complaint(complaintId("c"));
  const stop = autorun(() => {
    void room.current;
    void complaint.current;
  });
  await vi.advanceTimersByTimeAsync(100);
  expect(watch.mock.calls.map(([ref]) => ref)).toEqual([
    { kind: "room", roomId: "r" },
    { kind: "complaint", complaintId: "c" },
  ]);
  createRoomMirror(page); // No accessor/runtime is used by a mirror.
  expect(watch).toHaveBeenCalledTimes(2);
  stop();
  await vi.advanceTimersByTimeAsync(21_000);
  expect(release).toHaveBeenCalledTimes(2);
  store.dispose();
  subscriptions.dispose();
  vi.useRealTimers();
});

it("notifies room mirror roots for membership, reorder and legacy transitions", () => {
  const mirror = createRoomMirror([]);
  const ids: string[][] = [];
  const stop = autorun(() => {
    const current = mirror.current;
    ids.push(
      ("messages" in current ? current.messages : current).map(
        (item) => item.id,
      ),
    );
  });
  const x = {
    id: "x",
    at: 1,
    author: "a",
    room: "r",
    text: "X",
    deliveries: {},
  };
  const y = { ...x, id: "y" };
  mirror.apply([x, y]);
  const first = legacyItems(mirror)[0];
  mirror.apply([y, x]);
  expect(legacyItems(mirror)[1]).toBe(first);
  mirror.apply([x]);
  mirror.apply(page);
  mirror.apply([x]);
  expect(ids).toEqual([[], ["x", "y"], ["y", "x"], ["x"], ["m"], ["x"]]);
  stop();
});

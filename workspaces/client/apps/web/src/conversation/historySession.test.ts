import { expect, it } from "vitest";
import { autorun } from "mobx";
import { createTranscriptMirror } from "../generated/observableResources.ts";
import { createMessageViews } from "./messageView.ts";

it("retains generated record and input references without reading text for membership", () => {
  const mirror = createTranscriptMirror({
    items: [{ id: "a", role: "assistant", text: "old" }],
    truncated: false,
  });
  const views = createMessageViews("agent");
  let lists = 0;
  let texts = 0;
  let selected = views(mirror.current.items);
  const stopList = autorun(() => {
    lists++;
    selected = views(mirror.current.items);
  });
  const first = selected[0];
  const stopText = autorun(() => {
    texts++;
    void selected[0]!.text;
  });
  mirror.apply({
    items: [{ id: "a", role: "assistant", text: "new" }],
    truncated: false,
  });
  expect(selected[0]).toBe(first);
  expect(lists).toBe(1);
  expect(texts).toBe(2);
  expect(first!.text).toBe("new");
  expect({ ...first }.text).toBe("new");
  stopList();
  stopText();
});

import { TranscriptHistory } from "./historySession.ts";
import {
  mergeTranscript,
  MAX_TRANSCRIPT_PAGE_BYTES,
} from "../transcriptPageBounds.ts";
import type { TranscriptPageData } from "../types.ts";
import { vi } from "vitest";
import type { ResourceLiveDeltaEvent } from "../generated/observableResources.ts";

function historyFixture(dto: Parameters<typeof createTranscriptMirror>[0]) {
  const mirror = createTranscriptMirror(dto);
  let delta: (event: ResourceLiveDeltaEvent) => void = () => {};
  let connection: Parameters<
    ConstructorParameters<
      typeof TranscriptHistory
    >[3]["watchResourceConnection"]
  >[0] = () => {};
  const syncGet = vi.fn();
  const history = new TranscriptHistory(
    "agent",
    {
      get current() {
        return { status: "live" as const, data: mirror.current };
      },
    },
    { syncGet },
    {
      watchResourceConnection(accept) {
        connection = accept;
        return () => {};
      },
      watchLiveDeltas(accept) {
        delta = accept;
        return () => {};
      },
    },
  );
  const stop = history.start();
  return {
    mirror,
    history,
    syncGet,
    stop,
    delta: (event: ResourceLiveDeltaEvent) => delta(event),
    reset: () => connection("degraded", "connecting", {}),
  };
}

it("a live delta touches only its item, and reset/durable replacement discard its ephemeral run", () => {
  const f = historyFixture({
    items: [
      { id: "a", role: "assistant", text: "a" },
      { id: "b", role: "assistant", text: "b" },
    ],
    truncated: false,
  });
  let lists = 0,
    first = 0,
    second = 0;
  const stops = [
    autorun(() => {
      lists++;
      void f.history.items;
    }),
    autorun(() => {
      first++;
      f.history.live.get("a");
    }),
    autorun(() => {
      second++;
      f.history.live.get("b");
    }),
  ];
  const references = f.history.items;
  f.delta({ agentId: "agent", itemId: "a", offset: 1, text: "😀" });
  expect([lists, first, second]).toEqual([1, 2, 1]);
  expect(f.history.items).toBe(references);
  f.mirror.apply({
    items: [
      { id: "a", role: "assistant", text: "a😀" },
      { id: "b", role: "assistant", text: "b" },
    ],
    truncated: false,
  });
  expect(f.history.live.get("a")).toBeUndefined();
  expect(f.history.items).toBe(references);
  f.delta({ agentId: "agent", itemId: "a", offset: 2, text: "c" });
  f.reset();
  expect(f.history.live.get("a")).toBeUndefined();
  f.stop();
  stops.forEach((stop) => stop());
});

it("history invalidation and disposal reject late page responses", async () => {
  const f = historyFixture({
    items: [{ id: "a", role: "assistant", text: "current" }],
    truncated: true,
    nextCursor: "a",
    historyVersion: "v1",
  });
  const response = Promise.withResolvers<TranscriptPageData>();
  f.syncGet.mockReturnValueOnce(response.promise);
  const pending = f.history.ensureMessage("older");
  f.mirror.apply({
    items: [{ id: "new", role: "assistant", text: "fresh" }],
    truncated: false,
    historyVersion: "v2",
  });
  response.resolve({
    items: [{ id: "older", role: "assistant", text: "stale" }],
    truncated: false,
    historyVersion: "v1",
  });
  expect(await pending).toBe(false);
  expect(f.history.items.map((item) => item.id)).toEqual(["new"]);
  expect(f.history.pageLoading).toBe(false);
  const second = Promise.withResolvers<TranscriptPageData>();
  f.syncGet.mockReturnValueOnce(second.promise);
  const late = f.history.ensureMessage("lost");
  f.stop();
  second.resolve({
    items: [{ id: "lost", role: "assistant", text: "stale" }],
    truncated: false,
  });
  expect(await late).toBe(false);
  expect(f.history.items[0]?.text).toBe("fresh");
});

it("recalculates estimated UTF-16 page bytes after an in-place model update", () => {
  const rows = Array.from({ length: 240 }, (_, index) => ({
    id: String(index),
    role: "assistant",
    text: "a",
    at: index,
  }));
  const f = historyFixture({ items: rows, truncated: true, nextCursor: "0" });
  f.mirror.apply({
    items: rows.map((row, index) =>
      index === 239 ? { ...row, text: "x".repeat(1_999_000) } : row,
    ),
    truncated: true,
    nextCursor: "0",
  });
  expect(f.history.size).toBeLessThanOrEqual(MAX_TRANSCRIPT_PAGE_BYTES);
  expect(f.history.items.length).toBeLessThan(240);
  expect(f.history.items.at(-1)?.text).toBe("x".repeat(1_999_000));
  f.stop();
});

it("keeps server page order across equal and backdated timestamps without changing text bytes", () => {
  const a = { id: "a", role: "assistant", text: "😀\r\n\t", at: 30 };
  const b = { id: "b", role: "assistant", text: "b\u0000", at: 20 };
  const c = { id: "c", role: "assistant", text: "c", at: 20 };
  expect(mergeTranscript([a, b], [b, c])).toEqual([a, b, c]);
});

it("input views keep identity and native order as identified inputs move and change", () => {
  const mirror = createTranscriptMirror({
    items: [
      {
        id: "batch",
        role: "user",
        text: "",
        assets: [],
        inputs: [
          { id: "one", kind: "user", truncated: false, text: "one" },
          { id: "two", kind: "user", truncated: false, text: "two" },
        ],
      },
    ],
    truncated: false,
  });
  const views = createMessageViews("agent");
  const first = views(mirror.current.items);
  mirror.apply({
    items: [
      {
        id: "batch",
        role: "user",
        text: "",
        assets: [],
        inputs: [
          { id: "two", kind: "user", truncated: false, text: "two changed" },
          { id: "one", kind: "user", truncated: false, text: "one" },
        ],
      },
    ],
    truncated: false,
  });
  const reordered = views(mirror.current.items);
  expect(reordered).toEqual([first[1], first[0]]);
  expect(reordered[0]?.text).toBe("two changed");
  expect(reordered.map((item) => item.id)).toEqual(["agent:two", "agent:one"]);
});

it("focused paging keeps overlapping live model references and streams into their leaves", async () => {
  const f = historyFixture({
    items: [{ id: "answer", role: "assistant", text: "now", streaming: true }],
    truncated: true,
    nextCursor: "answer",
    historyVersion: "v1",
  });
  const live = f.history.items[0];
  f.syncGet.mockResolvedValueOnce({
    items: [
      { id: "old", role: "user", text: "earlier" },
      { id: "answer", role: "assistant", text: "now", streaming: true },
    ],
    truncated: false,
    historyVersion: "v1",
  });
  expect(await f.history.ensureMessage("old")).toBe(true);
  expect(f.history.historical).toBe(true);
  expect(f.history.items[1]).toBe(live);
  f.mirror.apply({
    items: [
      { id: "answer", role: "assistant", text: "now updated", streaming: true },
    ],
    truncated: true,
    nextCursor: "answer",
    historyVersion: "v1",
  });
  expect(f.history.items[1]).toBe(live);
  expect(f.history.items[1]?.text).toBe("now updated");
  f.stop();
});

it("a backdated live tail preserves the retained prefix proven contiguous by overlap", async () => {
  const { updateLiveTranscriptPage } =
    await import("../transcriptPageBounds.ts");
  const a = { id: "a", role: "assistant", text: "earlier", at: 300 };
  const b = { id: "b", role: "assistant", text: "overlap", at: 200 };
  const c = { id: "c", role: "assistant", text: "later", at: 100 };
  const page = { items: [a, b], before: null, focused: false };
  updateLiveTranscriptPage(page, [b, c], { truncated: true, nextCursor: "b" });
  expect(page.items).toEqual([a, b, c]);
});

import { RoomHistory } from "./roomHistory.ts";
import { createRoomMirror } from "../generated/observableResources.ts";
const roomRow = (seq: number, text = `message ${seq}`) => ({
  id: `message-${seq}`,
  seq,
  room: "room",
  sender: "agent",
  senderName: "Agent",
  created: seq,
  text,
  deliveries: {},
});

it("room history retains generated leaves, bounds mutable bytes and rejects late page results", async () => {
  const mirror = createRoomMirror({
    room: { id: "room" },
    messages: [roomRow(1)],
    nextBefore: 1,
    nextAfter: 2,
  });
  const syncGet = vi.fn();
  const room = new RoomHistory(
    "room",
    {
      get current() {
        return { status: "live" as const, data: mirror.current };
      },
    },
    { syncGet },
    false,
  );
  const stop = room.start();
  const first = room.items[0];
  mirror.apply({
    room: { id: "room" },
    messages: [roomRow(1, "changed"), roomRow(2)],
    nextBefore: 1,
    nextAfter: 2,
  });
  expect(room.items[0]).toBe(first);
  expect(first?.text).toBe("changed");
  expect(room.items.map((item) => item.seq)).toEqual([1, 2]);
  const pending =
    Promise.withResolvers<Parameters<typeof createRoomMirror>[0]>();
  syncGet.mockReturnValueOnce(pending.promise);
  const loading = room.older();
  stop();
  pending.resolve({
    room: { id: "room" },
    messages: [roomRow(0)],
    nextBefore: null,
  });
  await loading;
  expect(room.items.map((item) => item.seq)).toEqual([1, 2]);
});

it("legacy room facade root membership changes reach the session without a caller root mirror", () => {
  const legacy = (id: string, text: string) => ({
    id,
    text,
    at: 1,
    room: "room",
    author: "user",
    deliveries: {},
  });
  const mirror = createRoomMirror([legacy("one", "old")]);
  const room = new RoomHistory(
    "room",
    {
      get current() {
        return { status: "live" as const, data: mirror.current };
      },
    },
    { syncGet: vi.fn() },
    true,
  );
  const stop = room.start();
  const first = room.items[0];
  mirror.apply([legacy("one", "new"), legacy("two", "next")]);
  expect(room.items[0]).toBe(first);
  expect(first?.text).toBe("new");
  expect(room.items.map((item) => item.id)).toEqual(["one", "two"]);
  stop();
});

it("room paging rejects a forward cursor that cannot make progress", async () => {
  const mirror = createRoomMirror({
    room: { id: "room" },
    messages: [roomRow(1)],
    nextAfter: 1,
  });
  const syncGet = vi.fn().mockResolvedValue({
    room: { id: "room" },
    messages: [roomRow(1)],
    nextAfter: 1,
  });
  const room = new RoomHistory(
    "room",
    {
      get current() {
        return { status: "live" as const, data: mirror.current };
      },
    },
    { syncGet },
    false,
  );
  const stop = room.start();
  await expect(room.newer()).rejects.toThrow("cursor did not advance");
  expect(room.pageLoading).toBe(false);
  stop();
});

it("disposing a pending page changes observed UI state inside an action", async () => {
  const f = historyFixture({
    items: [{ id: "current", role: "assistant", text: "current" }],
    truncated: false,
  });
  const response = Promise.withResolvers<TranscriptPageData>();
  f.syncGet.mockReturnValueOnce(response.promise);
  const observeLoading = autorun(() => void f.history.pageLoading);
  const pending = f.history.ensureMessage("older");
  expect(f.history.pageLoading).toBe(true);
  const warnings = vi.spyOn(console, "warn").mockImplementation(() => {});
  try {
    f.stop();
    expect(warnings).not.toHaveBeenCalled();
    expect(f.history.pageLoading).toBe(false);
  } finally {
    observeLoading();
    warnings.mockRestore();
    response.resolve({ items: [], truncated: false });
    await pending;
  }
});

it("restarts a room page after leaving while its first request is pending", async () => {
  const dto = { room: { id: "room" }, messages: [roomRow(2)], nextBefore: 2 };
  const mirror = createRoomMirror(dto);
  const pending = Promise.withResolvers<typeof dto>();
  const syncGet = vi
    .fn()
    .mockReturnValueOnce(pending.promise)
    .mockResolvedValue(dto);
  const room = new RoomHistory(
    "room",
    {
      get current() {
        return { status: "live" as const, data: mirror.current };
      },
    },
    { syncGet },
    false,
  );
  const leave = room.start();
  const first = room.older();
  leave();
  const stop = room.start();
  try {
    await room.older();
    expect(syncGet).toHaveBeenCalledTimes(2);
  } finally {
    stop();
    pending.resolve(dto);
    await first;
  }
});

it.each(["reload", "newer"] as const)(
  "keeps canonical room records after %s overlap",
  async (operation) => {
    const dto = {
      room: { id: "room" },
      messages: [roomRow(2)],
      nextBefore: 2,
      nextAfter: 3,
    };
    const mirror = createRoomMirror(dto);
    const syncGet = vi.fn().mockResolvedValue({
      ...dto,
      messages: [roomRow(2, "reloaded")],
      nextAfter: 4,
    });
    const room = new RoomHistory(
      "room",
      {
        get current() {
          return { status: "live" as const, data: mirror.current };
        },
      },
      { syncGet },
      false,
    );
    const stop = room.start();
    try {
      const canonical = room.items[0];
      await room[operation]();
      mirror.apply({
        ...dto,
        messages: [roomRow(2, "authoritative-new-text")],
      });
      expect(room.items[0]).toBe(canonical);
      expect(room.items[0]?.text).toBe("authoritative-new-text");
    } finally {
      stop();
    }
  },
);

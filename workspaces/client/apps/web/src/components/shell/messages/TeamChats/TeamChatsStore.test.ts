// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { observableFixture } from "../../integration-support/observableFixture.tsx";
import { agentSummaryFixture } from "../../../../servers/agentSummaryFixture.ts";
import { createApiStorage } from "../../../../apiStorage.ts";
import { createTeamChatsStore } from "./TeamChatsStore.ts";
import type { StateEntity } from "../../../../generated/observableResources.ts";

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => void values.delete(key),
    setItem: (key, value) => void values.set(key, String(value)),
  };
}

it("keeps selection, search, paging and read acknowledgements with their scope", () => {
  const fixture = observableFixture();
  const storage = createApiStorage(memoryStorage());
  const save = vi.spyOn(storage, "save");
  const key = "codex-chat-seen:s8-seen";
  const entities: StateEntity[] = [
    {
      collection: "agentSummary",
      id: "lead",
      value: agentSummaryFixture("lead"),
    },
    ...Array.from({ length: 65 }, (_, index) => ({
      collection: "room" as const,
      id: String(index),
      value: {
        id: String(index),
        name: "Room " + index,
        kind: "broadcast" as const,
        rootId: "lead",
        lastMessage: {
          seq: 2,
          created: index,
          sender: "lead",
          text: "Preview " + index,
        },
      },
    })),
  ];
  try {
    storage.save(key, { other: 7 });
    save.mockClear();
    fixture.apply(entities);
    const chats = createTeamChatsStore(
      fixture.store,
      storage,
      "s8-seen",
      "lead",
    );
    expect(chats.filtered).toHaveLength(65);
    expect(chats.filtered[0]?.id).toBe("64");
    chats.more();
    expect(chats.limit).toBe(120);
    chats.search("PREVIEW 12");
    expect(chats.filtered.map((room) => room.id)).toEqual(["12"]);
    expect(chats.limit).toBe(60);
    chats.focus("12");
    expect(chats.detail).toBe(true);
    expect(chats.room?.id).toBe("12");
    chats.markSeen(chats.room, false, true, 2);
    chats.markSeen(chats.room, true, false, 2);
    expect(save).not.toHaveBeenCalled();
    chats.markSeen(chats.room, true, true, 2);
    expect(save).toHaveBeenCalledExactlyOnceWith(key, { other: 7, "12": 2 });
    chats.markSeen(chats.room, true, true, 2);
    expect(save).toHaveBeenCalledTimes(1);
    chats.back();
    expect(chats.detail).toBe(false);
    expect(chats.selected).toBe("12");
    const other = createTeamChatsStore(fixture.store, storage, "other", "lead");
    expect(other.selected).toBe("you");
    expect(other.query).toBe("");
    expect(other.seen.size).toBe(0);
    chats.focus(undefined, "complaint");
    expect(chats.selected).toBe("you");
    expect(chats.detail).toBe(true);
  } finally {
    fixture.store.dispose();
  }
});

it("reads generated updates and keeps session reads when the storage adapter swallows a write failure", () => {
  const fixture = observableFixture();
  const device = memoryStorage();
  const storage = createApiStorage(device);
  const room = {
    id: "remote",
    name: "Remote",
    kind: "federated" as const,
    localMembers: ["worker"],
    peerLabel: "Peer",
    lastMessage: { seq: 3, created: 1, sender: "worker", text: "First" },
  };
  const entities: StateEntity[] = [
    {
      collection: "agentSummary",
      id: "lead",
      value: agentSummaryFixture("lead"),
    },
    {
      collection: "agentSummary",
      id: "worker",
      value: { ...agentSummaryFixture("worker"), rootId: "lead" },
    },
    { collection: "room", id: room.id, value: room },
  ];
  try {
    fixture.apply(entities);
    const chats = createTeamChatsStore(
      fixture.store,
      storage,
      "s8-failed",
      "lead",
    );
    chats.search("first");
    expect(chats.filtered).toHaveLength(1);
    chats.choose(room.id);
    vi.spyOn(device, "setItem").mockImplementationOnce(() => {
      throw new Error("Storage unavailable");
    });
    expect(() => chats.markSeen(chats.room, true, true, 3)).not.toThrow();
    expect(chats.seen.get(room.id)).toBe(3);
    expect(storage.saved("codex-chat-seen:s8-failed", {})).toEqual({});
    fixture.apply([
      ...entities.slice(0, 2),
      {
        collection: "room",
        id: room.id,
        value: {
          ...room,
          lastMessage: { ...room.lastMessage, text: "Second" },
        },
      },
    ]);
    expect(chats.filtered).toHaveLength(0);
    chats.search("second");
    expect(chats.filtered).toHaveLength(1);
    expect(chats.name(chats.filtered[0]!)).toBe("Peer");
    chats.markSeen(chats.room, true, true, 3);
    expect(chats.seen.get(room.id)).toBe(3);
    fixture.apply([
      ...entities.slice(0, 2),
      {
        collection: "room",
        id: room.id,
        value: { ...room, localMembers: ["outside"] },
      },
    ]);
    expect(chats.rooms).toHaveLength(0);
  } finally {
    fixture.store.dispose();
  }
});

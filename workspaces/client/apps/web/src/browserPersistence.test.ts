import { expect, it, vi } from "vitest";
import {
  outboxRequestKeys,
  outboxRequests,
  scopedSessionOutbox,
} from "./browserPersistence.ts";

it("keeps typed outbox operations at schema-fenced keys", () => {
  const values = new Map<string, string>([
    ["studio-native-action:old-schema:workspace:agent", "old-schema"],
  ]);
  const storage = {
    get length() {
      return values.size;
    },
    key: (index: number) => [...values.keys()][index] ?? null,
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value);
    },
    removeItem: (key: string) => {
      values.delete(key);
    },
  } as Storage;
  const outbox = outboxRequests(storage);
  expect(outbox.get("nativeAction", "workspace:agent")).toBeNull();
  outbox.set("nativeAction", "workspace:agent", "body");
  expect(values.get(outbox.key("nativeAction", "workspace:agent"))).toBe(
    "body",
  );
  expect(outbox.get("nativeAction", "workspace:agent")).toBe("body");
  expect(outboxRequestKeys(storage, "nativeAction", "workspace:")).toEqual([
    "workspace:agent",
  ]);
  outbox.remove("nativeAction", "workspace:agent");
  expect(values.size).toBe(1);
});

it.each([
  ["nativeAction", "studio-native-action:"],
  ["queueChange", "studio-queue-change:"],
] as const)(
  "reads the exact previous %s request fence without writing",
  (kind, prefix) => {
    const legacyKey = prefix + "workspace:agent";
    const raw = '{ "request_id": "original", "unknown": true }';
    const values = new Map([[legacyKey, raw]]);
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
      removeItem: (key: string) => {
        values.delete(key);
      },
    };
    const outbox = outboxRequests(storage);
    expect(outbox.get(kind, "workspace:agent")).toBe(raw);
    expect(values.get(outbox.key(kind, "workspace:agent"))).toBeUndefined();
    expect(values.get(legacyKey)).toBe(raw);
    expect(outboxRequests(storage).get(kind, "workspace:agent")).toBe(raw);
  },
);

it.each(["nativeAction", "queueChange"] as const)(
  "reads and retains the previous %s fence when quota blocks writes",
  (kind) => {
    const prefix =
      kind === "nativeAction"
        ? "studio-native-action:"
        : "studio-queue-change:";
    const legacyKey = prefix + "workspace:agent";
    const raw = '{"request_id":"original"}';
    const values = new Map([[legacyKey, raw]]);
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: () => {
        throw new Error("storage full");
      },
      removeItem: vi.fn(),
    };
    const outbox = outboxRequests(storage);
    expect(outbox.get(kind, "workspace:agent")).toBe(raw);
    expect(() => outbox.set(kind, "workspace:agent", raw)).toThrow(
      "storage full",
    );
    expect(values.get(legacyKey)).toBe(raw);
    expect(storage.removeItem).not.toHaveBeenCalled();
  },
);

it.each([
  ["nativeAction", "studio-native-action:"],
  ["queueChange", "studio-queue-change:"],
] as const)(
  "acknowledges both %s keys without resurrecting a fence",
  (kind, prefix) => {
    const legacyKey = prefix + "workspace:agent";
    const values = new Map([[legacyKey, "original"]]);
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
      removeItem: (key: string) => {
        values.delete(key);
      },
    };
    const outbox = outboxRequests(storage);
    outbox.set(kind, "workspace:agent", "current");
    expect(values.get(legacyKey)).toBe("original");
    expect(outbox.get(kind, "workspace:agent")).toBe("current");
    outbox.remove(kind, "workspace:agent");
    expect(outbox.get(kind, "workspace:agent")).toBeNull();
    expect(values.size).toBe(0);
  },
);

it.each([
  ["nativeAction", "studio-native-action:"],
  ["queueChange", "studio-queue-change:"],
] as const)(
  "does not overwrite a second tab's newer %s fence during a read",
  (kind, prefix) => {
    const legacyKey = prefix + "workspace:agent";
    const values = new Map([[legacyKey, "original"]]);
    let interleave = true;
    const storage = {
      getItem: (key: string): string | null => {
        const value = values.get(key) ?? null;
        if (key === legacyKey && interleave) {
          interleave = false;
          // Tab B completes R and saves S after tab A captured legacy R.
          tabB.remove(kind, "workspace:agent");
          tabB.set(kind, "workspace:agent", "newer");
        }
        return value;
      },
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
      removeItem: (key: string) => {
        values.delete(key);
      },
    };
    const tabA = outboxRequests(storage);
    const tabB = outboxRequests(storage);
    expect(tabA.get(kind, "workspace:agent")).toBe("original");
    expect(tabB.get(kind, "workspace:agent")).toBe("newer");
    expect(values.has(legacyKey)).toBe(false);
  },
);

it("retains the existing per-server session prefix", () => {
  const values = new Map<string, string>();
  vi.stubGlobal("sessionStorage", {
    get length() {
      return values.size;
    },
    key: (index: number) => [...values.keys()][index] ?? null,
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    removeItem: (key: string) => values.delete(key),
  });
  try {
    const scoped = scopedSessionOutbox(":server:remote:");
    scoped.setItem("request", "exact body");
    expect(values.get(":server:remote:request")).toBe("exact body");
    expect(scoped.getItem("request")).toBe("exact body");
  } finally {
    vi.unstubAllGlobals();
  }
});

it("replays an existing monitor restart fence without changing its bytes", () => {
  let saved = '{"request_id":"old","outputPatterns":["ready"]}';
  const storage = {
    getItem: (key: string) =>
      key === "monitor-restart:remote:task" ? saved : null,
    setItem: (_key: string, value: string) => {
      saved = value;
    },
    removeItem: () => {
      saved = "";
    },
  };
  const outbox = outboxRequests(storage);
  expect(outbox.get("monitorRestart", "remote:task")).toBe(saved);
});

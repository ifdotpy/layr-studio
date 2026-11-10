import { expect, it, vi } from "vitest";
import {
  outboxRequestKeys,
  outboxRequests,
  scopedSessionOutbox,
} from "./browserPersistence.ts";

it("keeps typed outbox operations at schema-fenced keys", () => {
  const values = new Map<string, string>([
    ["studio-native-action:workspace:agent", "old-format"],
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
  expect(values.size).toBe(2);
});

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

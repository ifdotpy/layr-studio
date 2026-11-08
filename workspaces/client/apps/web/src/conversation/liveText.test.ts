import { describe, expect, it } from "vitest";
import { createLiveText, withLiveText, MAX_LIVE_ITEMS } from "./liveText.ts";

const item = "agent:answer";
const run = (...deltas: [number, string][]) => {
  const live = createLiveText();
  for (const [offset, text] of deltas) live.add({ itemId: item, offset, text });
  return live;
};

describe("live text", () => {
  it("shows live text on top of a durable item that lags behind it", () => {
    const live = run([0, "Hel"], [3, "lo"]).get(item);
    expect(withLiveText("", live)).toBe("Hello");
    expect(withLiveText("Hel", live)).toBe("Hello");
  });

  it("is replaced by the durable item when that is at least as long", () => {
    const live = run([0, "Hel"], [3, "lo"]).get(item);
    expect(withLiveText("Hello", live)).toBe("Hello");
    expect(withLiveText("Hello world", live)).toBe("Hello world");
  });

  it("ignores live text that the durable item is too short to join", () => {
    const live = run([5, " world"]).get(item);
    expect(withLiveText("He", live)).toBe("He");
    expect(withLiveText("Hello", live)).toBe("Hello world");
  });

  it("starts a new run after a lost delta", () => {
    const live = run([0, "Hel"], [10, "end"]).get(item);
    expect(live).toEqual({ start: 10, length: 3, text: "end" });
  });

  it("counts Unicode scalars like the server", () => {
    const live = run([0, "a😀"], [2, "b"]).get(item);
    expect(withLiveText("a", live)).toBe("a😀b");
    expect(withLiveText("a😀", live)).toBe("a😀b");
  });

  it("keeps a bounded number of items", () => {
    const live = createLiveText();
    for (let index = 0; index < 20; index++)
      live.add({ itemId: `i${index}`, offset: 0, text: "x" });
    expect(live.get(`i${20 - MAX_LIVE_ITEMS - 1}`)).toBeUndefined();
    for (let index = 20 - MAX_LIVE_ITEMS; index < 20; index++)
      expect(live.get(`i${index}`)?.text).toBe("x");
  });
});

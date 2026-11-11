// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { action, observable } from "mobx";
import { afterEach, expect, it, vi } from "vitest";
import type { TokenRate as Rate } from "../../../usage/tokenRate.ts";
import TokenRate from "./TokenRate.tsx";

const rates = vi.hoisted(() => ({ read: (): Rate | null => null }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerTokenRates: () => ({ getTokenRate: rates.read }),
  }),
);
vi.mock("../../../hooks/useVisualActivity.ts", () => ({
  useVisualActivity: () => [() => {}, true],
}));
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

it("animates only the DOM label, snaps for reduced motion, and cancels on unmount", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.spyOn(performance, "now").mockReturnValue(1000);
  const source = observable.box<Rate>({
    agentId: "chat",
    rootId: "chat",
    turnId: "turn",
    active: true,
    estimated: false,
    rate: 20,
    outputTokens: 40,
  });
  rates.read = () => source.get();
  let motion = false;
  let changed = () => {};
  const remove = vi.fn();
  vi.stubGlobal("matchMedia", () => ({
    get matches() {
      return motion;
    },
    addEventListener: (_event: string, listener: () => void) => {
      changed = listener;
    },
    removeEventListener: remove,
  }));
  const frames = new Map<number, FrameRequestCallback>();
  let next = 0;
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    frames.set(++next, callback);
    return next;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
  const host = document.createElement("div");
  const root = createRoot(host);
  const render = () =>
    root.render(
      createElement(TokenRate, { agent: { id: "chat", turnId: "turn" } }),
    );
  try {
    await act(async () => render());
    expect(host.textContent).toBe("20 tok/s");
    await act(async () =>
      action(() => source.set({ ...source.get(), rate: 40 }))(),
    );
    expect(frames.size).toBe(1);
    // A new target must not flash before the animation's first frame.
    expect(host.textContent).toBe("20 tok/s");
    const tick = frames.get(next);
    if (!tick) throw new Error("Missing animation frame");
    frames.delete(next);
    tick(1225);
    expect(host.textContent).toBe("38 tok/s");
    motion = true;
    changed();
    expect(host.textContent).toBe("40 tok/s");
    expect(host.querySelector("span")?.dataset.reducedMotion).toBe("true");
    expect(frames.size).toBe(0);
    motion = false;
    await act(async () =>
      action(() => source.set({ ...source.get(), rate: 60 }))(),
    );
    expect(frames.size).toBe(1);
  } finally {
    await act(async () => root.unmount());
    host.remove();
  }
  expect(frames.size).toBe(0);
  expect(remove).toHaveBeenCalled();
});

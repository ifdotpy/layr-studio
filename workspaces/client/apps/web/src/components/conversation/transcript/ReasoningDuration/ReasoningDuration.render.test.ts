// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { observable, runInAction } from "mobx";
import { expect, it, vi } from "vitest";
import ReasoningDuration from "./ReasoningDuration.tsx";
vi.mock("../../../../hooks/useVisualActivity.ts", () => ({
  useVisualActivity: () => [null, true],
}));
it("resets the clock baseline when an observed reasoning timestamp advances in place", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.useFakeTimers();
  vi.setSystemTime(100000);
  const item = observable({
    id: "reasoning",
    reasoningSince: 100,
    reasoningObservedAt: 100,
    reasoningMs: 0,
  });
  const host = document.createElement("div"),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(createElement(ReasoningDuration, { item })),
    );
    await act(async () => vi.advanceTimersByTimeAsync(10000));
    expect(host.textContent).toContain("Thinking 10s");
    await act(async () =>
      runInAction(() => {
        item.reasoningObservedAt = 110;
      }),
    );
    expect(host.textContent).toContain("Thinking 10s");
  } finally {
    await act(async () => root.unmount());
    vi.useRealTimers();
    vi.unstubAllGlobals();
  }
});

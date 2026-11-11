// @vitest-environment happy-dom
import { act, createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, expect, it, vi } from "vitest";
import { createRoot } from "react-dom/client";
import { enableStaticRendering } from "mobx-react-lite";
import { autorun } from "mobx";
import { now } from "../../../../clock.ts";
import SettledDelivery from "./SettledDelivery.tsx";

const render = (transient: boolean) => {
  enableStaticRendering(true);
  try {
    return renderToStaticMarkup(
      createElement(SettledDelivery, {
        transient,
        children: createElement("span", null, "Status"),
      }),
    );
  } finally {
    enableStaticRendering(false);
  }
};

it("shows an outcome at once", () => {
  expect(render(false)).toBe("<span>Status</span>");
});

it("holds back a transient step on its first frame", () => {
  expect(render(true)).toBe("");
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

it("settles through the shared clock, resets for a new transient phase, and releases ticks", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.useFakeTimers();
  const host = document.createElement("div");
  const root = createRoot(host);
  const show = (transient: boolean) =>
    act(async () =>
      root.render(
        createElement(SettledDelivery, { transient, children: "Status" }),
      ),
    );
  try {
    await show(true);
    expect(host.textContent).toBe("");
    await act(async () => vi.advanceTimersByTimeAsync(1000));
    expect(host.textContent).toBe("Status");
    await show(false);
    expect(host.textContent).toBe("Status");
    await show(true);
    expect(host.textContent).toBe("");
    await act(async () => vi.advanceTimersByTimeAsync(1000));
    expect(host.textContent).toBe("Status");
  } finally {
    await act(async () => root.unmount());
  }
  expect(vi.getTimerCount()).toBe(0);
});

it("reveals at its deadline when another observer started the clock 100 ms earlier", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.useFakeTimers();
  const stopClock = autorun(() => now());
  await vi.advanceTimersByTimeAsync(100);
  const host = document.createElement("div");
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(SettledDelivery, { transient: true, children: "Status" }),
      ),
    );
    await act(async () => vi.advanceTimersByTimeAsync(999));
    expect(host.textContent).toBe("");
    await act(async () => vi.advanceTimersByTimeAsync(1));
    expect(host.textContent).toBe("Status");
  } finally {
    await act(async () => root.unmount());
    stopClock();
  }
  expect(vi.getTimerCount()).toBe(0);
});

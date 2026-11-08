import { expect, it, vi } from "vitest";
import { _getGlobalState, autorun, computed, observable, spy } from "mobx";
const mocks = vi.hoisted(() => ({ unmount: vi.fn(), render: vi.fn() }));
vi.mock("react-dom/client", () => ({
  createRoot: () =>
    new (class {
      render = mocks.render;
      unmount() {
        mocks.unmount();
      }
    })(),
}));
import { createRoot } from "./root.tsx";
it("enables MobX diagnostics with warning-only strict checks", () => {
  expect(_getGlobalState()).toMatchObject({
    enforceActions: true,
    computedRequiresReaction: true,
    reactionRequiresObservable: true,
    observableRequiresReaction: true,
  });
  const warning = vi.spyOn(console, "warn").mockImplementation(() => {});
  const events = vi.fn();
  const stopSpy = spy(events);
  const value = observable.box(1);
  const stop = autorun(() => value.get());
  try {
    expect(() => value.set(2)).not.toThrow();
    expect(value.get()).toBe(2);
    expect(computed(() => value.get()).get()).toBe(2);
    autorun(() => {})();
    expect(events).toHaveBeenCalled();
    for (const text of [
      "strict-mode",
      "Computed value",
      "Observable",
      "without reading any observable",
    ]) {
      expect(warning.mock.calls.flat().join("\n")).toContain(text);
    }
  } finally {
    stop();
    stopSpy();
    warning.mockRestore();
  }
});
it("preserves root unmount", () => {
  vi.stubGlobal("document", { createElement: () => ({}) });
  try {
    createRoot(document.createElement("div")).unmount();
    expect(mocks.unmount).toHaveBeenCalledOnce();
  } finally {
    vi.unstubAllGlobals();
  }
});

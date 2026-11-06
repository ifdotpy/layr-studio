import { expect, it, vi } from "vitest";
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
import { createRoot } from "./root";
it("preserves root unmount", () => {
  vi.stubGlobal("document", { createElement: () => ({}) });
  try {
    createRoot(document.createElement("div")).unmount();
    expect(mocks.unmount).toHaveBeenCalledOnce();
  } finally {
    vi.unstubAllGlobals();
  }
});

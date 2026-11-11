// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import MoveToBackgroundButton from "./MoveToBackgroundButton.tsx";
const h = vi.hoisted(() => ({
  api: { name: "first" },
  request: vi.fn(
    async (
      _api: unknown,
      _agent: string,
      _turn: string | undefined,
      _item: string,
    ) => ({ kind: "backgrounded" as const }),
  ),
}));
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({ useServerApi: () => h.api }),
);
vi.mock("../support/commandBackground.ts", () => ({
  requestCommandBackground: h.request,
  checkCommandBackground: vi.fn(),
}));
it("a mounted command control uses the current target and server after props change", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(MoveToBackgroundButton, {
          agentId: "first",
          turnId: "turn-1",
          itemId: "item-1",
        }),
      ),
    );
    await act(async () => host.querySelector("button")!.click());
    expect(host.querySelector("button")!.disabled).toBe(true);
    h.api = { name: "second" };
    await act(async () =>
      root.render(
        createElement(MoveToBackgroundButton, {
          agentId: "second",
          turnId: "turn-2",
          itemId: "item-2",
        }),
      ),
    );
    expect(host.querySelector("button")!.disabled).toBe(false);
    await act(async () => host.querySelector("button")!.click());
    expect(h.request).toHaveBeenLastCalledWith(
      h.api,
      "second",
      "turn-2",
      "item-2",
    );
  } finally {
    await act(async () => root.unmount());
    vi.unstubAllGlobals();
  }
});

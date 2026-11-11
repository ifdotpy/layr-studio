// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { NetStatus } from "./status.ts";

it("keeps the network status receiver when its method is a button handler", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const status = new NetStatus();
  const host = document.createElement("div");
  const root = createRoot(host);
  const failures: unknown[] = [];
  const onError = (event: ErrorEvent) => {
    failures.push(event.error);
    event.preventDefault();
  };
  window.addEventListener("error", onError);
  try {
    await act(async () =>
      root.render(
        createElement("button", { onClick: status.begin }, "Begin request"),
      ),
    );
    const button = host.querySelector("button");
    expect(button).not.toBeNull();
    await act(async () => button!.click());
    expect(failures).toEqual([]);
    expect(status.inFlight).toBe(1);
  } finally {
    await act(async () => root.unmount());
    window.removeEventListener("error", onError);
    vi.unstubAllGlobals();
  }
});

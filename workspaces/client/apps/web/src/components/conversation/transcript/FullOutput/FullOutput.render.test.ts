// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, expect, it, vi } from "vitest";
import FullOutput from "./FullOutput.tsx";
import { clearFullOutputCache } from "./fullOutputCache.ts";

const { get } = vi.hoisted(() => ({ get: vi.fn() }));
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({ useServerApi: () => ({ get }) }),
);
afterEach(() => {
  clearFullOutputCache();
  get.mockReset();
  vi.unstubAllGlobals();
});

it("opens on click, retries failure and shares the content read across mounted leaves", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div");
  const root = createRoot(host);
  const blob = { hash: "shared-output", size: 1024 };
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(FullOutput, { blob }),
          createElement(FullOutput, { blob }),
        ),
      ),
    );
    expect(get).not.toHaveBeenCalled();
    get.mockRejectedValueOnce(new Error("Read failed"));
    await act(async () => host.querySelector("button")?.click());
    expect(host.textContent).toContain("Read failed");
    get.mockResolvedValueOnce({ text: "Full command output" });
    await act(async () => host.querySelector("button")?.click());
    expect(get).toHaveBeenCalledTimes(2);
    expect(host.querySelectorAll("pre")).toHaveLength(2);
    expect(
      Array.from(host.querySelectorAll("pre"), (node) => node.textContent),
    ).toEqual(["Full command output", "Full command output"]);
  } finally {
    await act(async () => root.unmount());
  }
});

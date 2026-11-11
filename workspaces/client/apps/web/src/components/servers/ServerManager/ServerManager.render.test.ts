// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { ServerDiscovery } from "../../../servers/useServerDiscovery.ts";
import ServerManager from "./ServerManager.tsx";

vi.mock("../ServerAccessSettings/ServerAccessSettings.tsx", () => ({
  default: () => null,
}));

it.each(["pending", "rejected"])(
  "renders a %s discovery request through the same mounted owner",
  async (outcome) => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    const request = Promise.withResolvers<void>();
    const discovery = new ServerDiscovery(
      {
        api: { refreshSession: vi.fn(), serverAccess: vi.fn() },
        store: { multiServer: () => ({ current: { status: "loading" } }) },
      },
      true,
      true,
      () => ({}),
    );
    const run = vi
      .spyOn(discovery.management, "run")
      .mockReturnValue(request.promise);
    const host = document.body.appendChild(document.createElement("div"));
    const root = createRoot(host);
    try {
      await act(async () =>
        root.render(
          createElement(
            MantineProvider,
            { env: "test" },
            createElement(ServerManager, {
              servers: [],
              add() {},
              remove() {},
              discovery,
              localEnabled: true,
            }),
          ),
        ),
      );
      const find = [...host.querySelectorAll("button")].find((button) =>
        button.textContent?.includes("Find servers now"),
      );
      if (!find) throw new Error("Missing discovery action");
      expect(find.disabled).toBe(false);
      await act(async () => find.click());
      expect(run).toHaveBeenCalledOnce();
      if (outcome === "pending") expect(find.disabled).toBe(true);
      else {
        await act(async () =>
          request.reject(new Error("Discovery request rejected")),
        );
        expect(host.querySelector('[role="alert"]')?.textContent).toContain(
          "Discovery request rejected",
        );
        expect(find.disabled).toBe(false);
      }
    } finally {
      await act(async () => {
        request.reject(new Error("Request finished"));
        root.unmount();
      });
      host.remove();
      run.mockRestore();
      vi.unstubAllGlobals();
    }
  },
);

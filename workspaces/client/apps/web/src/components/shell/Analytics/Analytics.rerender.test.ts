import { AnalyticsScope } from "../../../generated/api.ts";
// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, createElement as h } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { ReadSlot } from "../../../store/runtime.ts";
import type { AnalyticsResult } from "../../../analytics/AnalyticsStore.ts";

const scope = vi.hoisted(() => ({
  owner: { id: "first-server" },
  reads: { analytics: vi.fn() },
  exportAnalytics: vi.fn(),
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerNet: () => scope,
    useServerOwner: () => scope.owner,
  }),
);
import Analytics from "./Analytics.tsx";
import { time } from "../../../analytics/format.ts";

it("uses the new target, live server and close callback after a mounted rerender", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const firstClose = vi.fn(),
    secondClose = vi.fn();
  const read: ReadSlot<AnalyticsResult> = {
    current: { status: "live", data: {} },
    read: vi.fn(async () => ({})),
  };
  scope.reads.analytics.mockReturnValue(read);
  const render = (agent: string, onClose: () => void, slot = read) =>
    root.render(
      h(
        MantineProvider,
        { env: "test" },
        h(Analytics, {
          agent: { id: agent, name: agent },
          selection: {
            query: { agent, scope: AnalyticsScope.agent },
            read: slot,
          },
          onClose,
        }),
      ),
    );
  try {
    await act(async () => render("first", firstClose));
    const replacement: ReadSlot<AnalyticsResult> = {
      ...read,
      current: { status: "live", data: { generatedAt: 1_800_000_120 } },
    };
    await act(async () => render("first", firstClose, replacement));
    expect(document.querySelector(".analytics-scope")?.textContent).toContain(
      time(1_800_000_120),
    );
    scope.owner.id = "second-server";
    await act(async () => render("second", secondClose));
    const refresh = document.querySelector<HTMLButtonElement>(
      '[aria-label="Refresh analytics"]',
    );
    expect(refresh).not.toBeNull();
    await act(async () => refresh?.click());
    expect(scope.reads.analytics).toHaveBeenLastCalledWith(
      expect.objectContaining({ agent: "second" }),
    );
    const close = document.querySelector<HTMLButtonElement>(
      ".mantine-Modal-close",
    );
    expect(close).not.toBeNull();
    await act(async () => close?.click());
    expect(firstClose).not.toHaveBeenCalled();
    expect(secondClose).toHaveBeenCalledOnce();
  } finally {
    await act(async () => root.unmount());
    host.remove();
    scope.reads.analytics.mockReset();
    vi.unstubAllGlobals();
  }
});

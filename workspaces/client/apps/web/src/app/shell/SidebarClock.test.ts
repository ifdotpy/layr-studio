import { setStartupFixture } from "../../components/observableFixture.tsx";
// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { runInAction } from "mobx";
import Sidebar from "../../components/Sidebar.tsx";
import { SidebarListOwner } from "./SidebarOwner.ts";
import {
  conversationActionsFixture,
  observableFixture,
} from "../../components/observableFixture.tsx";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";
import { ServerStoreProvider } from "../../servers/ServerServicesContext.tsx";
import { COMPACT_RECENT_WINDOW_SECONDS } from "../../generated/sharedValues.ts";
import { componentSnapshot as snapshotFixture } from "../../components/componentTestFixtures.ts";
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
it.each([1, 1.0005])(
  "expires a rendered compact row after a %s second deadline without a server update",
  async (remaining) => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-07T12:00:00Z"));
    const models = observableFixture();
    models.apply([
      {
        collection: "agentSummary",
        id: "recent",
        value: agentSummaryFixture("recent", {
          projectPath: "/project",
          name: "Recent chat",
          updated:
            Date.now() / 1000 - COMPACT_RECENT_WINDOW_SECONDS + remaining,
        }),
      },
      {
        collection: "project",
        id: "project",
        value: { id: "project", path: "/project", name: "Project" },
      },
    ]);
    const a = conversationActionsFixture(() => {}, models.store);
    runInAction(() => {
      setStartupFixture(
        a,
        Object.assign(snapshotFixture(), {
          token: "",
          stateDir: "/fixture",
        }),
      );
      a.sidebar.mobileOpen = true;
      a.preferences.value = {
        ...a.preferences.value,
        hideOldChatsThreshold: 1,
      };
    });
    const owner = new SidebarListOwner(a);
    const stop = owner.start(),
      host = document.createElement("div"),
      root = createRoot(host);
    document.body.append(host);
    try {
      await act(async () =>
        root.render(
          createElement(
            MantineProvider,
            null,
            createElement(
              ServerStoreProvider,
              { value: models.store },
              createElement(Sidebar, { owner }),
            ),
          ),
        ),
      );
      expect(
        document.body.querySelector('[data-chat="recent"]'),
      ).not.toBeNull();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1001);
      });
      expect(document.body.querySelector('[data-chat="recent"]')).toBeNull();
    } finally {
      await act(async () => root.unmount());
      stop();
      a.dispose();
      a.scope.services.close();
      models.store.dispose();
      host.remove();
    }
  },
);

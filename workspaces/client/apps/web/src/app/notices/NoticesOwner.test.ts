import { mountConversationFixture } from "../../components/conversation/ConversationView/ConversationView.fixture.tsx";
import {
  setStartupFixture,
  setStartupWorkspace,
} from "../../components/shell/integration-support/observableFixture.tsx";
import { AgentSource } from "../../generated/api.ts";
import { runInAction } from "mobx";
import { enableStaticRendering } from "mobx-react-lite";
// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import App from "../../components/shell/App/App.tsx";
import { ServerAreasProvider } from "../../components/shell/ServerAreasProvider/ServerAreasProvider.tsx";
import { componentSnapshot as snapshotFixture } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { observableFixture } from "../../components/shell/integration-support/observableFixture.tsx";
import { conversationActionsFixture } from "../../components/shell/integration-support/observableFixture.tsx";

afterEach(() => vi.useRealTimers());
it("the app retains offline diagnostics until delivery settles and suppresses only their duplicate toast", async () => {
  vi.useFakeTimers();
  const models = observableFixture([
    { id: "lead", source: AgentSource.managed, isLead: true },
  ]);
  const areas = conversationActionsFixture(() => {}, models.store);
  setStartupFixture(
    areas,
    Object.assign(snapshotFixture(), {
      token: "",
      stateDir: "/fixture",
    }),
  );
  setStartupWorkspace(areas, "workspace");
  areas.navigation.selectedId = "lead";
  areas.delivery.entries = [
    {
      id: "send",
      body: { id: "send", room: "lead", text: "first" },
      status: {
        source: "device",
        state: { kind: "transport_failed", error: "Failed to fetch" },
      },
      created: 1,
    },
  ];
  areas.delivery.online = false;
  areas.delivery.retainDiagnostics();
  areas.startup.startupError = "Failed to fetch";
  mountConversationFixture(areas);
  const markup = () =>
    renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(
          models.Provider,
          null,
          createElement(ServerAreasProvider, {
            areas,
            children: createElement(App),
          }),
        ),
      ),
    );
  enableStaticRendering(true);
  try {
    areas.notices.notify("Failed to fetch");
    expect(markup()).not.toContain('id="toast"');
    expect(markup()).not.toContain('id="error"');
    runInAction(() => {
      areas.delivery.online = true;
    });
    areas.delivery.retainDiagnostics();
    expect(areas.delivery.offlinePending).toBe(true);
    areas.notices.notify("A distinct operation failed");
    expect(markup()).toContain("A distinct operation failed");
    runInAction(() => {
      areas.delivery.entries = [];
    });
    areas.delivery.retainDiagnostics();
    expect(areas.delivery.offlinePending).toBe(false);
  } finally {
    enableStaticRendering(false);
    areas.dispose();
    areas.scope.services.close();
    models.store.dispose();
  }
  // Flush deferred observer registration while the fixture clock still owns it.
  await Promise.resolve();
  vi.runAllTicks();
  expect(vi.getTimerCount()).toBe(0);
});
it("the disposed composition ignores late command notifications", async () => {
  vi.useFakeTimers();
  const areas = conversationActionsFixture();
  try {
    areas.notices.notify("first");
    await vi.advanceTimersByTimeAsync(5000);
    expect(areas.notices.toast).toBe("");
    areas.dispose();
    areas.notices.notify("late");
    expect(areas.notices.toast).toBe("");
    expect(vi.getTimerCount()).toBe(0);
  } finally {
    areas.dispose();
    areas.scope.services.close();
    areas.scope.store.dispose();
  }
});

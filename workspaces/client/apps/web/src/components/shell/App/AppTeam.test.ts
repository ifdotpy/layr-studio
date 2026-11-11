import { ProviderKind } from "../../../generated/providerKind.ts";
import { createComplaintMirror } from "../../../generated/observableResources.ts";
import { SidebarListOwner } from "../../../app/shell/SidebarOwner.ts";
import * as frameBridge from "../../../servers/frameBridge.ts";
// @vitest-environment happy-dom
import { StrictMode, act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { configure } from "mobx";
import { expect, it, vi } from "vitest";
import { AppTeam } from "../AppTeam/AppTeam.tsx";
import { AgentSource, AgentStatus } from "../../../generated/api.ts";
import { ServerAreasContextProvider } from "../../../app/ServerAreasProvider.tsx";
import { useTeam } from "../../agents/support/TeamPanelContext.tsx";
import {
  conversationActionsFixture,
  observableFixture,
} from "../integration-support/observableFixture.tsx";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";

it("keeps team controls stable while selected-worker status and team membership update their leaves", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => true,
  }));
  configure({ enforceActions: "observed" });
  const lead = agentSummaryFixture("lead", {
    isLead: true,
    source: AgentSource.managed,
  });
  const worker = agentSummaryFixture("worker", {
    isLead: false,
    rootId: "lead",
    source: AgentSource.managed,
    status: AgentStatus.running,
  });
  const fixture = observableFixture([lead, worker]);
  const areas = conversationActionsFixture(() => {}, fixture.store);
  areas.navigation.selectExternal("worker", { requestId: "initial" });
  let controls = 0;
  let team: ReturnType<typeof useTeam> | undefined;
  function Controls() {
    controls++;
    team = useTeam();
    return null;
  }
  const Selected = observer(() => {
    const value = useTeam();
    return createElement(
      "span",
      null,
      `${value.opened}:${value.completedOpen}`,
    );
  });
  const host = document.createElement("div"),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          StrictMode,
          null,
          createElement(
            fixture.Provider,
            null,
            createElement(
              ServerAreasContextProvider,
              { value: areas },
              createElement(AppTeam, {
                children: [
                  createElement(Controls, { key: "controls" }),
                  createElement(Selected, { key: "selected" }),
                ],
              }),
            ),
          ),
        ),
      ),
    );
    const initial = controls,
      actions = team;
    expect(host.textContent).toBe("worker:false");
    await act(async () =>
      fixture.mirror.apply([
        { collection: "agentSummary", id: "lead", value: lead },
        {
          collection: "agentSummary",
          id: "worker",
          value: { ...worker, status: AgentStatus.completed },
        },
      ]),
    );
    expect(host.textContent).toBe("worker:true");
    expect(controls).toBe(initial);
    await act(async () =>
      fixture.mirror.apply([
        { collection: "agentSummary", id: "lead", value: lead },
        {
          collection: "agentSummary",
          id: "worker",
          value: { ...worker, status: AgentStatus.completed },
        },
        {
          collection: "agentSummary",
          id: "second",
          value: { ...worker, id: "second" },
        },
      ]),
    );
    expect(controls).toBe(initial);
    expect(team).toBe(actions);
    await act(async () => team?.setCompletedOpen(false));
    expect(host.textContent).toBe("worker:false");
    const remove = team?.remove;
    await act(async () =>
      areas.navigation.selectExternal("second", { requestId: "second" }),
    );
    await act(async () =>
      areas.navigation.selectExternal("worker", { requestId: "worker-again" }),
    );
    expect(host.textContent).toBe("worker:true");
    expect(team?.remove).toBe(remove);
  } finally {
    await act(async () => root.unmount());
    areas.dispose();
    areas.scope.services.close();
    fixture.store.dispose();
    vi.unstubAllGlobals();
  }
});

function ownerFixture() {
  vi.spyOn(SidebarListOwner.prototype, "start").mockReturnValue(() => {});
  vi.spyOn(frameBridge, "startAppFrame").mockReturnValue(() => {});
  const f = observableFixture([
    agentSummaryFixture("lead", { isLead: true, source: AgentSource.managed }),
  ]);
  const areas = conversationActionsFixture(() => {}, f.store);
  return {
    f,
    areas,
    close(stop: () => void) {
      stop();
      areas.scope.services.close();
      f.store.dispose();
      vi.restoreAllMocks();
    },
  };
}

it("drops a confirmed complaint cache entry after authoritative catch-up", () => {
  const { f, areas, close } = ownerFixture();
  const detail: Parameters<typeof createComplaintMirror>[0] = {
    id: "complaint",
    recipient: "user",
    version: 1,
    text: "Question",
    responses: [],
  };
  const mirror = createComplaintMirror(detail);
  vi.spyOn(f.store, "complaint").mockReturnValue({
    get current(): ReturnType<typeof f.store.complaint>["current"] {
      return { status: "live", data: mirror.current };
    },
  });
  const stop = areas.ui.start();
  const key = JSON.stringify(["scope", detail.id]);
  try {
    areas.ui.rememberComplaint(key, { ...detail, version: 3 });
    mirror.apply({ ...detail, version: 2 });
    expect(areas.ui.messageCache.has(key)).toBe(true);
    mirror.apply({ ...detail, version: 3 });
    expect(areas.ui.messageCache.has(key)).toBe(false);
  } finally {
    close(stop);
  }
});

it("clears a disposed activity owner when selection has no lead", () => {
  const { areas, close } = ownerFixture();
  areas.navigation.selectExternal("lead", { requestId: "lead" });
  areas.ui.openActivity();
  const stop = areas.ui.start();
  try {
    const old = areas.ui.activity;
    expect(old).toBeDefined();
    areas.navigation.selectExternal("missing", { requestId: "missing" });
    expect(areas.ui.tasksOpen).toBe(true);
    expect(old?.disposed).toBe(true);
    expect(areas.ui.activity).toBeUndefined();
    areas.navigation.selectExternal("lead", { requestId: "return" });
    areas.ui.closeTasks();
    areas.navigation.selectExternal("missing", { requestId: "closed-missing" });
    areas.ui.openActivity();
    expect(areas.ui.activity).toBeUndefined();
  } finally {
    close(stop);
  }
});

import { createScreenChannel } from "../../../servers/screenChannel.ts";
import { ScreenOwner } from "../../../app/startup/ScreenOwner.ts";
import { accountsFixture } from "../../accounts/Accounts/Accounts.fixture.ts";

it("owns terminal commands and publication across selection, visibility and disposal without a dock render", async () => {
  const { areas, close } = ownerFixture();
  const screen = new ScreenOwner();
  const publish = vi.fn();
  const channel = createScreenChannel("fixture", publish);
  screen.connect(channel, () => true);
  Object.defineProperty(areas.scope, "screen", { value: screen });
  Object.defineProperty(areas.scope.services.sync, "projectionMetadata", {
    value: { ready: true },
  });
  areas.preferences.value.showTerminals = true;
  areas.layout.recordResize({
    ...areas.layout.geometry,
    viewport: { x: 0, y: 0, width: 1400, height: 900 },
  });
  areas.navigation.selectExternal("lead", { requestId: "initial" });
  const unbind = channel.bind(() => {});
  const post = vi
    .spyOn(areas.ui.terminal.actions, "post")
    .mockRejectedValue(new Error("lost create"));
  const notify = vi.fn();
  areas.notices.notify = notify;
  const stop = areas.ui.start();
  areas.ui.terminal.measure(40);
  try {
    await channel.execute({ action: "terminal", operation: "toggle" });
    expect(publish).toHaveBeenLastCalledWith(
      expect.objectContaining({
        kind: "studio-server-terminal",
        opened: true,
        canCreate: true,
      }),
    );
    await channel.execute({ action: "terminal", operation: "new" });
    await Promise.resolve();
    expect(post).toHaveBeenCalledWith(
      "/api/terminals/create",
      expect.objectContaining({ agent: "lead" }),
    );
    expect(notify).toHaveBeenCalledWith("lost create");
    areas.navigation.selectExternal("missing", { requestId: "missing" });
    await channel.execute({ action: "terminal", operation: "new" });
    expect(post).toHaveBeenCalledTimes(1);
    expect(publish).toHaveBeenLastCalledWith(
      expect.objectContaining({ canCreate: false }),
    );
    areas.preferences.value = {
      ...areas.preferences.value,
      showTerminals: false,
    };
    await expect(
      channel.execute({ action: "terminal", operation: "toggle" }),
    ).rejects.toThrow("terminal is unavailable");
    areas.preferences.value = {
      ...areas.preferences.value,
      showTerminals: true,
    };
    const replacement = createScreenChannel("replacement", publish);
    const unbindReplacement = replacement.bind(() => {});
    screen.connect(replacement, () => true);
    await expect(
      channel.execute({ action: "terminal", operation: "toggle" }),
    ).rejects.toThrow("terminal is unavailable");
    await replacement.execute({ action: "terminal", operation: "toggle" });
    expect(publish).toHaveBeenLastCalledWith(
      expect.objectContaining({ serverId: "replacement", opened: false }),
    );
    screen.connect(replacement, () => false);
    await expect(
      replacement.execute({ action: "terminal", operation: "toggle" }),
    ).rejects.toThrow("terminal is unavailable");
    unbindReplacement();
  } finally {
    close(stop);
    unbind();
  }
  const calls = publish.mock.calls.length;
  await expect(
    channel.execute({ action: "terminal", operation: "toggle" }),
  ).rejects.toThrow();
  expect(publish).toHaveBeenCalledTimes(calls);
});

it("reports account dialogs from their owner and closes external settings after the dialog closes", () => {
  vi.spyOn(SidebarListOwner.prototype, "start").mockReturnValue(() => {});
  vi.spyOn(frameBridge, "startAppFrame").mockReturnValue(() => {});
  const fixture = accountsFixture();
  const { areas } = fixture;
  const stop = areas.ui.start();
  try {
    areas.settings.run({
      action: "account-sign-in",
      forceAdd: true,
      provider: ProviderKind.Codex,
      email: null,
      label: "New",
      serverLabel: "Fixture",
    });
    expect(areas.accountsUi.dialogOpen).toBe(true);
    expect(areas.settings.command).toBeNull();
    expect(areas.settings.accountModalOpen).toBe(true);
    expect(areas.settings.studioOpen).toBe(true);
    areas.accountsUi.close();
    expect(areas.settings.accountModalOpen).toBe(false);
    expect(areas.settings.externalAccount).toBe(false);
    expect(areas.settings.studioOpen).toBe(false);
    areas.accountsUi.openAdd({ provider: ProviderKind.Codex });
    expect(areas.settings.accountModalOpen).toBe(true);
  } finally {
    stop();
    fixture.close();
    vi.restoreAllMocks();
  }
  expect(areas.settings.accountModalOpen).toBe(false);
});

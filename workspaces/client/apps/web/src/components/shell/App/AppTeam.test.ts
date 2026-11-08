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

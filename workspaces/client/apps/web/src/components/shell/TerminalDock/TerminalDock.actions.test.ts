// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, createElement, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { Provider } from "../../../app/ServerAreasProvider.tsx";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import {
  conversationActionsFixture,
  observableFixture,
} from "../integration-support/observableFixture.tsx";
import TerminalDock from "./TerminalDock.tsx";

const captured = vi.hoisted(() => ({ values: [] as unknown[] }));
vi.mock("../../../app/area.ts", async (original) => {
  const actual = await original<typeof import("../../../app/area.ts")>();
  return {
    createArea<T>(name: string) {
      const [Provider, useArea] = actual.createArea<T>(name);
      return name === "Terminal"
        ? [
            ({
              value,
              children,
            }: {
              value: T | null;
              children?: ReactNode;
            }) => {
              captured.values.push(value);
              return createElement(Provider, { value }, children);
            },
            useArea,
          ]
        : [Provider, useArea];
    },
  };
});

it("provides the same terminal actions when the selected agent changes", async () => {
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
  const fixture = observableFixture([
    agentSummaryFixture("first"),
    agentSummaryFixture("second"),
  ]);
  const areas = conversationActionsFixture(undefined, fixture.store);
  const host = document.body.appendChild(document.createElement("div")),
    root = createRoot(host);
  const render = (id: string) => {
    areas.navigation.selectExternal(id, { requestId: id });
    root.render(
      createElement(
        fixture.Provider,
        null,
        createElement(
          MantineProvider,
          null,
          createElement(
            Provider,
            { value: areas },
            createElement(TerminalDock),
          ),
        ),
      ),
    );
  };
  try {
    await act(async () => render("first"));
    const first = captured.values.at(-1),
      count = captured.values.length;
    await act(async () => render("second"));
    expect(captured.values.length).toBeGreaterThan(count);
    expect(captured.values.at(-1)).toBe(first);
  } finally {
    await act(async () => root.unmount());
    host.remove();
    areas.dispose();
    areas.scope.services.close();
    fixture.store.dispose();
    vi.unstubAllGlobals();
  }
});

import { TerminalState } from "../../../generated/api.ts";
import { createTerminalMirror } from "../../../generated/observableResources.ts";
import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";

it("a mounted dock follows replacement server and UI owners", async () => {
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
  const cases = ["first owner", "replacement owner"].map((title) => {
    const fixture = observableFixture([]);
    const terminal = createTerminalMirror({
      id: title,
      agent: "lead",
      cwd: "/",
      title,
      status: TerminalState.running,
      created: 1,
    });
    const store: typeof fixture.store = {
      ...fixture.store,
      terminals: () => ({
        current: { status: "live", data: [terminal.current] },
      }),
    };
    const areas = conversationActionsFixture(undefined, store);
    areas.ui.terminal.open(true);
    return { fixture, store, areas };
  });
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const render = (source: (typeof cases)[number]) =>
    root.render(
      createElement(
        ServerStoreProvider,
        { value: source.store },
        createElement(
          MantineProvider,
          null,
          createElement(
            Provider,
            { value: source.areas },
            createElement(TerminalDock),
          ),
        ),
      ),
    );
  try {
    const first = cases[0]!,
      second = cases[1]!;
    await act(async () => render(first));
    expect(host.textContent).toContain("first owner");
    await act(async () => render(second));
    expect(host.textContent).toContain("replacement owner");
    expect(host.textContent).not.toContain("first owner");
    expect(captured.values.at(-1)).toMatchObject({
      notify: second.areas.notices.notify,
    });
  } finally {
    await act(async () => root.unmount());
    host.remove();
    for (const source of cases) {
      source.areas.dispose();
      source.areas.scope.services.close();
      source.store.dispose();
    }
    vi.unstubAllGlobals();
  }
});

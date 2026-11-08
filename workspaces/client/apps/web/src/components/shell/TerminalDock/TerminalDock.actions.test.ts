// @vitest-environment happy-dom
import { act, createElement, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { observableFixture } from "../integration-support/observableFixture.tsx";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";

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
import TerminalDock from "./TerminalDock.tsx";

it("provides the same terminal actions when an unrelated dock prop changes", async () => {
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
  const fixture = observableFixture();
  const host = document.body.appendChild(document.createElement("div")),
    root = createRoot(host);
  const notify = vi.fn();
  const render = (id: string) =>
    root.render(
      createElement(
        fixture.Provider,
        null,
        createElement(
          MantineProvider,
          null,
          createElement(TerminalDock, {
            notify,
            agent: agentSummaryFixture(id),
          }),
        ),
      ),
    );
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
    fixture.store.dispose();
    vi.unstubAllGlobals();
  }
});

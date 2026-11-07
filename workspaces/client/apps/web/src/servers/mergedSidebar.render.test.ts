import { ChatRowActionsContext } from "../components/chat-row/actions.ts";
// @vitest-environment happy-dom
import { act, createElement, Profiler } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SidebarRow from "../components/SidebarRow.tsx";
import {
  createMergedSidebarSelector,
  createSidebarSourceStore,
} from "./mergedSidebar.ts";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";
import { agentEntities } from "./sidebarSourceFixture.ts";
import { sidebarSnapshot } from "./sidebarSnapshot.ts";
import { snapshotFixture } from "../snapshotFixture.ts";

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: () => true,
  }));
});
afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});
const noop = () => {};
const actions = {
  prepare: noop,
  open: noop,
  pin: noop,
  rename: noop,
  name: noop,
  cancelRename: noop,
  beginRename: noop,
  unread: noop,
  move: noop,
  changeProject: noop,
  archive: noop,
  remove: noop,
};
const bindings = {};

it("a combined source update renders only its changed row with overlapping chat ids", async () => {
  const agents = [agentSummaryFixture("same"), agentSummaryFixture("other")];
  const store = createSidebarSourceStore();
  const snapshot = (tail = "") =>
    sidebarSnapshot(snapshotFixture([]), [
      ...agentEntities(agents),
      { collection: "agentTail", id: "same", value: { id: "same", tail } },
    ]);
  store.update("local", snapshot());
  store.update("remote", snapshot());
  const sources = () =>
    store.sources(["local", "remote"], new Set(["local", "remote"]));
  const select = createMergedSidebarSelector();
  const first = select(sources());
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const render = (model: ReturnType<typeof select>) =>
    root.render(
      createElement(
        MantineProvider,
        null,
        model.rows.map((row) =>
          createElement(
            Profiler,
            {
              id: row.id,
              key: row.id,
              onRender: (id) => renders.set(id, (renders.get(id) || 0) + 1),
            },
            createElement(
              ChatRowActionsContext.Provider,
              { value: () => actions, key: row.id },
              createElement(SidebarRow, {
                key: row.id,
                row,
                selected: false,
                serverAlias: "Server",
                showServer: false,
                renaming: false,
                organizing: false,
                name: "",
                compact: false,
                markingRead: false,
                bindings,
              }),
            ),
          ),
        ),
      ),
    );
  const renders = new Map<string, number>();
  try {
    await act(async () => render(first));
    renders.clear();
    await act(async () => {
      store.update("remote", snapshot("changed search tail"));
      select(sources());
    });
    expect([...renders.values()]).toEqual([]);
    await act(async () => {
      store.update("remote", snapshot("changed search tail"));
      select(sources());
    });
    expect([...renders.values()]).toEqual([]);
    await act(async () => {
      store.update(
        "remote",
        sidebarSnapshot(
          snapshotFixture([]),
          agentEntities(
            agents.map((agent) =>
              agent.id === "same"
                ? {
                    ...agent,
                    name: "Changed remotely",
                    operationalStatus: { reason: "needs_input", wait: null },
                  }
                : agent,
            ),
          ),
        ),
      );
      select(sources());
    });
    expect([...renders.keys()]).toEqual([first.chatKey("remote", "same")]);
    expect(renders.get(first.chatKey("remote", "same"))).toBe(1);
    expect(host.textContent).toContain("Changed remotely");
  } finally {
    await act(async () => root.unmount());
  }
});

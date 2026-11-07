// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SidebarRow from "../components/SidebarRow.tsx";
import {
  createMergedSidebarSelector,
  type SidebarSource,
} from "./mergedSidebar.ts";
import { sidebarRowFixture } from "./sidebarRowFixture.ts";
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
  delete window.__studioPromptComposerRenderProbe;
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
  const source = (id: string): SidebarSource => ({
    id,
    online: true,
    sidebar: sidebarSnapshot(snapshotFixture([]), [
      sidebarRowFixture("same"),
      sidebarRowFixture("other"),
    ]),
  });
  const local = source("local"),
    remote = source("remote");
  const select = createMergedSidebarSelector();
  const first = select([local, remote]);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const render = (model: ReturnType<typeof select>) =>
    root.render(
      createElement(
        MantineProvider,
        null,
        model.rows.map((row) =>
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
            actions,
          }),
        ),
      ),
    );
  const renders = new Map<string, number>();
  window.__studioPromptComposerRenderProbe = (kind, id) => {
    if (kind === "sidebar-row" && id)
      renders.set(id, (renders.get(id) || 0) + 1);
  };
  try {
    await act(async () => render(first));
    renders.clear();
    await act(async () =>
      render(
        select([
          local,
          {
            ...remote,
            sidebar: {
              ...remote.sidebar,
              rows: remote.sidebar.rows.map((row) => ({ ...row })),
            },
          },
        ]),
      ),
    );
    expect([...renders.values()]).toEqual([]);
    await act(async () =>
      render(
        select([
          local,
          {
            ...remote,
            sidebar: {
              ...remote.sidebar,
              rows: remote.sidebar.rows.map((row) =>
                row.id === "same"
                  ? {
                      ...row,
                      name: "Changed remotely",
                      indicator: { kind: "answer", label: "Needs an answer" },
                    }
                  : { ...row },
              ),
            },
          },
        ]),
      ),
    );
    expect([...renders.keys()]).toEqual([first.chatKey("remote", "same")]);
    expect(renders.get(first.chatKey("remote", "same"))).toBe(1);
    expect(host.textContent).toContain("Changed remotely");
  } finally {
    await act(async () => root.unmount());
  }
});

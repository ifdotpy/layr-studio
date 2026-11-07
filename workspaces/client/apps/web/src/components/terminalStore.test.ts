// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { TerminalState } from "../generated/api.ts";
it("output on terminal A causes zero renders of terminal B's leaves", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => true,
    media: "",
    onchange: null,
  }));
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: { load: async () => [] },
  });
  const shell = (id: string) =>
    createTerminalMirror({
      id,
      agent: "lead",
      cwd: "/",
      title: id,
      status: TerminalState.running,
      created: 1,
    }).current;
  const initial = {
    text: "a🙂",
    offset: 2,
    status: TerminalState.running,
    truncated: false,
  } as const;
  const a = createTerminalOutputMirror(initial);
  const b = createTerminalOutputMirror(initial);
  const renders: string[] = [];
  const get = vi.spyOn(api, "get");
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    const views = (
      [
        ["A", a],
        ["B", b],
      ] as const
    ).map(([id, mirror]) =>
      createElement(
        Profiler,
        {
          key: id,
          id,
          onRender: (id) => {
            renders.push(id);
          },
        },
        createElement(ShellView, {
          shell: shell(id),
          outputSlot: { current: { status: "live", data: mirror.current } },
        }),
      ),
    );
    const mount = (count: number) =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            TerminalActionsProvider,
            { value: { ...api, notify() {} } },
            ...views.slice(0, count),
          ),
        ),
      );
    await act(async () => mount(1));
    await vi.waitFor(() => expect(writes).toEqual(["a🙂"]));
    await act(async () => mount(2));
    await vi.waitFor(() => expect(writes).toEqual(["a🙂", "a🙂"]));
    renders.length = 0;
    await act(async () => a.apply({ ...initial, text: "a🙂中", offset: 3 }));
    expect(renders.filter((id) => id === "A").length).toBeGreaterThan(0);
    expect(renders.filter((id) => id === "B")).toHaveLength(0);
    expect(writes.slice(-1)).toEqual(["中"]);
    await act(async () =>
      a.apply({
        ...initial,
        text: "a🙂中",
        offset: 3,
        status: TerminalState.exited,
      }),
    );
    expect(renders.filter((id) => id === "B")).toHaveLength(0);
    expect(host.textContent).toContain("Session ended");
    expect(get).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
    host.remove();
    get.mockRestore();
    vi.unstubAllGlobals();
  }
});

import { act, createElement, Profiler } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import {
  createTerminalMirror,
  createTerminalOutputMirror,
} from "../generated/observableResources.ts";
import { ShellView, TerminalActionsProvider } from "./TerminalDock.tsx";
import * as api from "../api.ts";
const { writes } = vi.hoisted(() => ({ writes: [] as string[] }));
vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    options = {};
    loadAddon() {}
    open() {}
    onData() {}
    reset() {}
    dispose() {}
    write(text: string) {
      writes.push(text);
    }
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));

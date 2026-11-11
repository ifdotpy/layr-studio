// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { TerminalState } from "../../../generated/api.ts";
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
    expect(renders.filter((id) => id === "A")).toHaveLength(0);
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
} from "../../../generated/observableResources.ts";
import { ShellView, TerminalActionsProvider } from "./TerminalDock.tsx";
import * as api from "../../../servers/net/api.ts";
const { writes, inputs } = vi.hoisted(() => ({
  writes: [] as string[],
  inputs: [] as ((text: string) => void)[],
}));
vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    options = {};
    loadAddon() {}
    open() {}
    onData(callback: (text: string) => void) {
      inputs.push(callback);
    }
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

import { afterEach } from "vitest";
import { flowResult } from "mobx";
import { TerminalDockStore } from "./TerminalDockStore.ts";
afterEach(() => vi.restoreAllMocks());

it("dock commands lock before sending, preserve rejected drafts and clear confirmations on selection", async () => {
  const post = vi
    .spyOn(api, "post")
    .mockRejectedValue(new Error("lost response"));
  const notify = vi.fn(),
    store = new TerminalDockStore({ ...api, notify });
  store.search("shell");
  store.rename("new title");
  store.confirmEnd("a");
  const attempt = flowResult(
    store.action("/api/terminals/rename", {
      id: "a",
      title: store.renameTitle ?? "",
    }),
  );
  expect(store.pending).toBe(true);
  await flowResult(store.action("/api/terminals/close", { id: "a" }));
  expect(post).toHaveBeenCalledTimes(1);
  await attempt;
  expect(store.pending).toBe(false);
  expect(store.renameTitle).toBe("new title");
  expect(notify).toHaveBeenCalledWith("lost response");
  store.select();
  expect(store.renameTitle).toBeNull();
  expect(store.endTarget).toBeNull();
  expect(store.query).toBe("shell");
});

it("dock rename success clears its draft without refreshing resources", async () => {
  const created = {
    id: "shell",
    agent: "lead",
    cwd: "/",
    title: "shell",
    status: TerminalState.running,
    created: 1,
  };
  vi.spyOn(api, "post").mockResolvedValue(created);
  const get = vi.spyOn(api, "get");
  const store = new TerminalDockStore({ ...api, notify: vi.fn() });
  store.search("old filter");
  store.rename("new title");
  await flowResult(
    store.action("/api/terminals/rename", { id: "shell", title: "new title" }),
  );
  expect(store.renameTitle).toBeNull();
  expect(get).not.toHaveBeenCalled();
});

it("a mounted shell pauses uncertain input, drops buffered input, and reconnects without resending it", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: { load: async () => [] },
  });
  const shell = createTerminalMirror({
    id: "input",
    agent: "lead",
    cwd: "/",
    title: "Input",
    status: TerminalState.running,
    created: 1,
  }).current;
  const output = {
    text: "",
    offset: 0,
    truncated: false,
    status: TerminalState.running,
  };
  const mirror = createTerminalOutputMirror(output);
  const sync = vi
    .spyOn(api, "syncPost")
    .mockRejectedValue(new Error("response lost"));
  const get = vi.spyOn(api, "get").mockResolvedValue(output);
  const host = document.body.appendChild(document.createElement("div")),
    root = createRoot(host);
  const beforeMount = inputs.length;
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            TerminalActionsProvider,
            { value: { ...api, notify: vi.fn() } },
            createElement(ShellView, {
              shell,
              outputSlot: { current: { status: "live", data: mirror.current } },
            }),
          ),
        ),
      ),
    );
    await vi.waitFor(() => expect(inputs.length).toBeGreaterThan(beforeMount));
    await act(async () => {
      inputs.at(-1)?.("first");
      inputs.at(-1)?.("buffered");
    });
    expect(sync).toHaveBeenCalledTimes(1);
    expect(sync.mock.calls[0]?.[1]).toMatchObject({
      id: "input",
      text: "first",
      request_id: expect.any(String),
    });
    expect(host.textContent).toContain("Buffered input was not sent");
    const previous = inputs.length;
    const reconnect = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "Reconnect",
    );
    expect(reconnect).toBeDefined();
    await act(async () => reconnect?.click());
    await vi.waitFor(() => expect(inputs.length).toBeGreaterThan(previous));
    expect(get).toHaveBeenCalledTimes(1);
    expect(sync).toHaveBeenCalledTimes(1);
    expect(host.textContent).toContain("Previous input delivery is uncertain");
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});

import type { GetResult } from "../../../api.ts";

it.each([
  {
    text: "first page",
    offset: 10,
    hasMore: true,
    truncated: false,
    status: TerminalState.running,
  },
  {
    text: "first page",
    offset: 10,
    availableOffset: 20,
    hasMore: true,
    truncated: false,
    status: TerminalState.running,
  },
  {
    text: "first page",
    offset: 10,
    availableOffset: 20,
    historyStart: 0,
    truncated: false,
    status: TerminalState.running,
  },
] satisfies GetResult<"/api/terminals/output">[])(
  "rejects incomplete saved history metadata before assembling a download: %j",
  async (response) => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    Object.defineProperty(document, "fonts", {
      configurable: true,
      value: { load: async () => [] },
    });
    const createObjectURL = vi.fn(() => "blob:history");
    vi.stubGlobal(
      "URL",
      class extends URL {
        static override createObjectURL = createObjectURL;
      },
    );
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    const get = vi.spyOn(api, "get").mockResolvedValue(response);
    const shell = createTerminalMirror({
      id: "history",
      agent: "lead",
      cwd: "/",
      title: "History",
      status: TerminalState.running,
      created: 1,
    }).current;
    const mirror = createTerminalOutputMirror({
      text: "",
      offset: 0,
      truncated: false,
      status: TerminalState.running,
    });
    const host = document.body.appendChild(document.createElement("div")),
      root = createRoot(host);
    try {
      await act(async () =>
        root.render(
          createElement(
            MantineProvider,
            null,
            createElement(
              TerminalActionsProvider,
              { value: { ...api, notify: vi.fn() } },
              createElement(ShellView, {
                shell,
                outputSlot: {
                  current: { status: "live", data: mirror.current },
                },
              }),
            ),
          ),
        ),
      );
      const download = [...host.querySelectorAll("button")].find(
        (button) => button.textContent === "Download saved output",
      );
      expect(download).toBeDefined();
      await act(async () => download?.click());
      expect(get).toHaveBeenCalledTimes(1);
      expect(get).toHaveBeenCalledWith("/api/terminals/output", {
        query: { id: "history", history: "1", offset: 0, limit: 65536 },
      });
      expect(host.textContent).toContain(
        "The server did not return saved terminal output.",
      );
      expect(createObjectURL).not.toHaveBeenCalled();
      expect(host.textContent).not.toContain("Saved output downloaded.");
    } finally {
      await act(async () => root.unmount());
      host.remove();
      vi.unstubAllGlobals();
    }
  },
);

it("a mounted shell reads replacement output and sends through replacement actions", async () => {
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
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: { load: async () => [] },
  });
  const shell = createTerminalMirror({
    id: "replacement",
    agent: "lead",
    cwd: "/",
    title: "Replacement",
    status: TerminalState.running,
    created: 1,
  }).current;
  const first = createTerminalOutputMirror({
    text: "first output",
    offset: 12,
    truncated: false,
    status: TerminalState.running,
  });
  const second = createTerminalOutputMirror({
    text: "replacement output",
    offset: 18,
    truncated: false,
    status: TerminalState.running,
  });
  const oldActions = { ...api, notify() {} };
  const newActions = { ...api, notify() {} };
  const pending = Promise.withResolvers<{ ok: boolean }>();
  const oldSend = vi
    .spyOn(oldActions, "syncPost")
    .mockReturnValue(pending.promise);
  const newSend = vi
    .spyOn(newActions, "syncPost")
    .mockResolvedValue({ ok: true });
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const render = (output: typeof first, actions: typeof oldActions) =>
    root.render(
      createElement(
        MantineProvider,
        null,
        createElement(
          TerminalActionsProvider,
          { value: actions },
          createElement(ShellView, {
            shell,
            outputSlot: { current: { status: "live", data: output.current } },
          }),
        ),
      ),
    );
  try {
    await act(async () => render(first, oldActions));
    await vi.waitFor(() => expect(writes).toContain("first output"));
    await act(async () => {
      inputs.at(-1)?.("pending input");
      inputs.at(-1)?.("buffered input");
    });
    expect(oldSend).toHaveBeenCalledTimes(1);
    await act(async () => render(second, newActions));
    expect(host.textContent).toContain("Previous input delivery is uncertain");
    expect(newSend).not.toHaveBeenCalled();
    expect(writes).toContain("replacement output");
    const interrupt = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "Ctrl+C",
    );
    await act(async () => interrupt?.click());
    expect(newSend).toHaveBeenCalledWith(
      "/api/terminals/input",
      expect.objectContaining({ id: shell.id, text: "\u0003" }),
    );
    await act(async () => pending.resolve({ ok: true }));
    expect(newSend).toHaveBeenCalledTimes(1);
    expect(oldSend).toHaveBeenCalledTimes(1);
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});

import { createShellTerminalStore } from "./ShellTerminalStore.ts";
it.each(["resolve", "reject"] as const)(
  "a late reconnect %s cannot replace a newer source",
  async (outcome) => {
    const shell = createTerminalMirror({
      id: "late-reconnect",
      agent: "lead",
      cwd: "/",
      title: "Reconnect",
      status: TerminalState.running,
      created: 1,
    }).current;
    const oldActions = { ...api },
      currentActions = { ...api };
    const pending = Promise.withResolvers<GetResult<"/api/terminals/output">>();
    vi.spyOn(oldActions, "get").mockReturnValue(pending.promise);
    const first = createTerminalOutputMirror({
      text: "first",
      offset: 5,
      truncated: false,
      status: TerminalState.running,
    });
    const next = createTerminalOutputMirror({
      text: "new",
      offset: 3,
      truncated: false,
      status: TerminalState.running,
    });
    const controller = createShellTerminalStore(
      shell,
      { current: { status: "live", data: first.current } },
      oldActions,
    );
    const request = flowResult(controller.reconnect());
    controller.bind(
      shell,
      { current: { status: "live", data: next.current } },
      currentActions,
    );
    if (outcome === "reject") pending.reject(new Error("old reconnect failed"));
    else
      pending.resolve({
        text: "old reconnect",
        offset: 13,
        truncated: false,
        status: TerminalState.running,
      });
    await request;
    expect(controller.output?.text).toBe("new");
    expect(controller.failure).toBe("");
  },
);

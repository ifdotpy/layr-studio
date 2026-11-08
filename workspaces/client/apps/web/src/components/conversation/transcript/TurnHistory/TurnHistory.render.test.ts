// @vitest-environment happy-dom
import { act, createElement, createRef } from "react";
import { configure, observable, runInAction } from "mobx";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { TranscriptProvider } from "../../support/navigation.ts";
import TurnHistory from "./TurnHistory.tsx";
import type { Agent, Message } from "../../../../types.ts";

vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerStore: () => ({}),
    useServerApi: () => ({}),
  }),
);
vi.mock("../Activity/Activity.tsx", async (original) => ({
  ...(await original<typeof import("../Activity/Activity.tsx")>()),
  ToolCard: ({ item, cwd }: { item: Message; cwd?: string }) =>
    createElement("span", { "data-tool": item.id, "data-cwd": cwd }),
}));
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  delete window.__studioPromptComposerRenderProbe;
  document.body.replaceChildren();
});

it("HistoryContent and TranscriptRow stay asleep on resize and scroll with stable actions", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.useFakeTimers();
  const observers: { callback: ResizeObserverCallback; targets: Element[] }[] =
    [];
  vi.stubGlobal(
    "ResizeObserver",
    class {
      entry: (typeof observers)[number];
      constructor(callback: ResizeObserverCallback) {
        this.entry = { callback, targets: [] };
        observers.push(this.entry);
      }
      observe(target: Element) {
        this.entry.targets.push(target);
      }
      disconnect() {}
    },
  );
  const host = document.createElement("div");
  document.body.append(host);
  Object.defineProperty(host, "clientHeight", { value: 300 });
  const scroll = createRef<HTMLDivElement>();
  scroll.current = host;
  const items: Message[] = Array.from({ length: 90 }, (_, i) => ({
    id: `${i}`,
    role: "user",
    text: `message ${i}`,
    turnId: `${i}`,
  }));
  let content = 0;
  const message = vi.fn((item: Message) =>
    createElement("p", { "data-message": item.id }, item.text),
  );
  window.__studioPromptComposerRenderProbe = (component) => {
    if (component === "history-row") content++;
  };
  const actions = { renderMessage: message, jumpToMessage() {} };
  const agent = observable<Agent>(
    { id: "lead", cwd: "/project" },
    {},
    { name: "RowAgent" },
  );
  configure({ observableRequiresReaction: true });
  const warnings = vi.spyOn(console, "warn").mockImplementation(() => {});
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          TranscriptProvider,
          { value: actions },
          createElement(TurnHistory, {
            items,
            enabled: false,
            storageKey: "row-render-counts",
            agent,
            scrollContainer: scroll,
          }),
        ),
      ),
    );
    await act(async () => vi.advanceTimersByTimeAsync(100));
    content = 0;
    message.mockClear();
    const resize = observers.findLast((entry) =>
      entry.targets.some((target) => target.hasAttribute("data-history-row")),
    )!;
    await act(async () =>
      resize.callback(
        [
          {
            target: resize.targets[0]!,
            borderBoxSize: [{ blockSize: 80, inlineSize: 500 }],
            contentBoxSize: [],
            devicePixelContentBoxSize: [],
            contentRect: new DOMRect(0, 0, 500, 80),
          } satisfies ResizeObserverEntry,
        ],
        new ResizeObserver(() => {}),
      ),
    );
    expect({
      HistoryContent: content,
      TranscriptRow: message.mock.calls.length,
    }).toEqual({ HistoryContent: 0, TranscriptRow: 0 });
    content = 0;
    message.mockClear();
    const body = host.querySelector<HTMLElement>("[data-history-window]")!;
    body.getBoundingClientRect = () =>
      new DOMRect(0, -host.scrollTop, 500, 90 * 72);
    const before = new Set(
      Array.from(
        host.querySelectorAll<HTMLElement>("[data-history-row]"),
        (row) => row.dataset.historyRow,
      ),
    );
    await act(async () => {
      host.scrollTop = 288;
      host.dispatchEvent(new Event("scroll"));
      await vi.advanceTimersByTimeAsync(100);
    });
    const entering = Array.from(
      host.querySelectorAll<HTMLElement>("[data-history-row]"),
      (row) => row.dataset.historyRow,
    ).filter((id) => !before.has(id));
    expect(entering.length).toBeGreaterThan(0);
    expect({
      HistoryContent: content,
      TranscriptRow: message.mock.calls.length,
    }).toEqual({
      HistoryContent: entering.length,
      TranscriptRow: entering.length,
    });
    expect(message.mock.calls.map(([item]) => item.id)).toEqual(entering);
    const cwdWarnings = warnings.mock.calls.filter(([text]) =>
      String(text).includes("RowAgent.cwd"),
    );
    console.log("row callback cwd warnings", cwdWarnings.length);
    expect(cwdWarnings).toHaveLength(0);
  } finally {
    await act(async () => root.unmount());
    warnings.mockRestore();
    configure({ observableRequiresReaction: false });
  }
});

it("failure metadata updates only its observed row without rerunning full grouping", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const items = [
    observable<Message>({
      id: "first",
      role: "assistant",
      text: "answer",
      turnId: "first",
      turnStatus: "uncertain",
      nativeError: { message: "old reason" },
    }),
    observable<Message>({
      id: "second",
      role: "assistant",
      text: "answer",
      turnId: "second",
      turnStatus: "completed",
    }),
  ];
  const renders: string[] = [];
  window.__studioPromptComposerRenderProbe = (component, id) => {
    renders.push(`${component}:${id}`);
  };
  const actions = {
    renderMessage: (item: Message) => createElement("p", null, item.text),
    jumpToMessage() {},
  };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          TranscriptProvider,
          { value: actions },
          createElement(TurnHistory, {
            items,
            enabled: true,
            storageKey: "failure-counts",
          }),
        ),
      ),
    );
    renders.length = 0;
    await act(async () =>
      runInAction(() => {
        items[0]!.nativeError = { message: "new reason" };
      }),
    );
    expect(host.textContent).toContain("new reason");
    expect(
      renders.filter(
        (render) =>
          render.startsWith("turn-history:") ||
          render.startsWith("history-groups:"),
      ),
    ).toEqual([]);
    expect(
      renders.filter((render) => render.startsWith("history-row:")),
    ).toEqual(["history-row:first"]);
  } finally {
    await act(async () => root.unmount());
  }
});

it("changing cwd updates the rows rendered through the retained row callback", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const agent = observable({ id: "lead", cwd: "/before" });
  const items: Message[] = ["first", "second"].map((id) => ({
    id,
    role: "tool",
    turnId: id,
    text: JSON.stringify({ type: "fileChange" }),
  }));
  const actions = { renderMessage: () => null, jumpToMessage() {} };
  const rows: string[] = [];
  window.__studioPromptComposerRenderProbe = (component, id) => {
    if (component === "history-row" && id) rows.push(id);
  };
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const cwds = () =>
    [...host.querySelectorAll<HTMLElement>("[data-tool]")].map(
      (node) => node.dataset.cwd,
    );
  try {
    await act(async () =>
      root.render(
        createElement(
          TranscriptProvider,
          { value: actions },
          createElement(TurnHistory, {
            items,
            agent,
            enabled: false,
            storageKey: "cwd-transition",
          }),
        ),
      ),
    );
    expect(cwds()).toEqual(["/before", "/before"]);
    rows.length = 0;
    await act(async () =>
      runInAction(() => {
        agent.cwd = "/after";
      }),
    );
    expect(cwds()).toEqual(["/after", "/after"]);
    expect(rows).toEqual(["first", "second"]);
  } finally {
    await act(async () => root.unmount());
    host.remove();
  }
});

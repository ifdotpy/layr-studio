import { TranscriptStore } from "../../support/TurnHistoryStore.ts";
// @vitest-environment happy-dom
import { act, createElement, createRef } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import HistoryWindow from "./HistoryWindow.tsx";
import { TranscriptProvider } from "../../support/navigation.ts";
import type { Message } from "../../../../types.ts";
import type { HistoryGroup } from "../../support/turnHistoryModel.ts";

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

it("resize and scroll retain mounted content and never reread message geometry", async () => {
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
  let reads = 0;
  const rows: HistoryGroup[] = Array.from({ length: 90 }, (_, i) => ({
    id: `${i}`,
    items: [
      {
        id: `${i}`,
        role: "user",
        get text() {
          reads++;
          return "message";
        },
      },
    ],
  }));
  const renders = new Map<string, number>();
  const render = (row: Message) => {
    renders.set(row.id, (renders.get(row.id) || 0) + 1);
    return createElement("p", { "data-message": row.id }, row.id);
  };
  const host = document.createElement("div");
  document.body.append(host);
  const scroll = createRef<HTMLDivElement>();
  scroll.current = host;
  const model = new TranscriptStore(() => ({
    items: rows.flatMap((row) => row.items),
    enabled: false,
    scrollContainer: scroll,
    storageKey: "geometry-counts",
  }));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          TranscriptProvider,
          { value: { renderMessage: render, jumpToMessage() {} } },
          createElement(HistoryWindow, { model }),
        ),
      ),
    );
    await act(async () => vi.advanceTimersByTimeAsync(100));
    const body = host.querySelector<HTMLElement>("[data-history-window]")!;
    body.getBoundingClientRect = () =>
      new DOMRect(0, -host.scrollTop, 500, 90 * 72);
    const before = new Map(renders);
    reads = 0;
    const rowObserver = observers.findLast((entry) =>
      entry.targets.some((target) => target.hasAttribute("data-history-row")),
    )!;
    const target = rowObserver.targets[0]!;
    await act(async () =>
      rowObserver.callback(
        [
          {
            target,
            borderBoxSize: [{ blockSize: 80, inlineSize: 500 }],
            contentBoxSize: [],
            devicePixelContentBoxSize: [],
            contentRect: new DOMRect(0, 0, 500, 80),
          } satisfies ResizeObserverEntry,
        ],
        new ResizeObserver(() => {}),
      ),
    );
    expect(reads, "resize geometry item reads").toBe(0);
    expect(renders).toEqual(before);
    reads = 0;
    await act(async () => {
      host.scrollTop = 100;
      host.dispatchEvent(new Event("scroll"));
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(reads, "scroll geometry item reads").toBe(0);
    for (const [id, count] of before) expect(renders.get(id)).toBe(count);
  } finally {
    await act(async () => root.unmount());
  }
});

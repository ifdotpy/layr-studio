import { AnalyticsScope } from "../../../generated/api.ts";
import { autorun, flowResult } from "mobx";
import { expect, it, vi } from "vitest";
import type { ReadSlot } from "../../../store/runtime.ts";
import {
  AnalyticsStore,
  type AnalyticsResult,
} from "../../../analytics/AnalyticsStore.ts";

it("keeps keyed read identities while deduplicating an older detail page", () => {
  const initial: ReadSlot<AnalyticsResult> = {
    current: { status: "live", data: {} },
    read: vi.fn(async () => ({})),
  };
  const page: ReadSlot<AnalyticsResult> = {
    current: { status: "live", data: {} },
    read: vi.fn(async () => ({})),
  };
  const analytics = vi.fn(() => page);
  const net = {
    reads: { analytics },
    exportAnalytics: vi.fn(async () => ({
      blob: new Blob(),
      name: "analytics.json",
      truncated: false,
    })),
  };
  const model = new AnalyticsStore({
    query: { agent: "agent", scope: AnalyticsScope.agent },
    read: initial,
  });
  const counts: number[] = [];
  const stop = autorun(() => counts.push(model.details.turns.length));
  try {
    model.loadOlderDetails(net, "turns");
    model.loadOlderDetails(net, "turns");
    expect(model.details.turns).toHaveLength(1);
    expect(model.details.turns[0]).toBe(page);
    expect(counts).toEqual([0, 1]);
    expect(analytics).toHaveBeenCalledWith({
      agent: "agent",
      scope: AnalyticsScope.agent,
      view: "detail",
      detail: "turns",
      limit: 100,
      offset: 0,
    });
  } finally {
    stop();
  }
});

it("keeps the requested scope in an export filename when filters change in flight", async () => {
  vi.useFakeTimers();
  const anchor = { href: "", download: "", click: vi.fn() };
  vi.stubGlobal("document", { createElement: () => anchor });
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:analytics");
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  let finish!: (value: {
    blob: Blob;
    name: string;
    truncated: boolean;
  }) => void;
  const read: ReadSlot<AnalyticsResult> = {
    current: { status: "live", data: {} },
    read: vi.fn(async () => ({})),
  };
  const net = {
    reads: { analytics: vi.fn(() => read) },
    exportAnalytics: vi.fn(
      () =>
        new Promise<{ blob: Blob; name: string; truncated: boolean }>(
          (resolve) => {
            finish = resolve;
          },
        ),
    ),
  };
  const model = new AnalyticsStore({
    query: { scope: AnalyticsScope.agent },
    read,
  });
  try {
    const pending = flowResult(model.download(net));
    model.load(net, { scope: AnalyticsScope.all });
    finish({ blob: new Blob(), name: "analytics.json", truncated: false });
    await pending;
    expect(net.exportAnalytics).toHaveBeenCalledWith({
      scope: AnalyticsScope.agent,
      export: "1",
    });
    expect(anchor.download).toContain("analytics-agent-");
  } finally {
    vi.runAllTimers();
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  }
});

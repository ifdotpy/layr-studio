import { describe, expect, it, vi } from "vitest";
import { createServerServices } from "../servers/serverServices";
import {
  collector,
  createCollector,
  startPerformanceReporting,
} from "./collector";

describe("performance reporting windows", () => {
  it("aggregates by event and component and resets after a report", () => {
    const collector = createCollector();
    collector.longTask(20);
    collector.longTask(30);
    collector.interaction("keydown", 25, 8);
    collector.interaction("keydown", 18, 10);
    collector.interaction("pointerdown", 17, 2);
    collector.render("composer", 4);
    collector.render("composer", 6);
    collector.render("history", 3);
    expect(collector.take()).toMatchObject({
      longTasks: { count: 2, totalMs: 50, maxMs: 30 },
      interactions: [
        { type: "keydown", count: 2, maxDurationMs: 25, maxInputDelayMs: 10 },
        {
          type: "pointerdown",
          count: 1,
          maxDurationMs: 17,
          maxInputDelayMs: 2,
        },
      ],
      renders: [
        { component: "composer", count: 2, totalMs: 10, maxMs: 6 },
        { component: "history", count: 1, totalMs: 3, maxMs: 3 },
      ],
    });
    expect(collector.take()).toBeUndefined();
  });
});

it("starts once and disposes observers and the reporting timer", async () => {
  vi.useFakeTimers();
  const observers: TestObserver[] = [];
  class TestObserver {
    static supportedEntryTypes = ["longtask", "event"];
    observe = vi.fn();
    disconnect = vi.fn();
    constructor() {
      observers.push(this);
    }
  }
  vi.stubGlobal("PerformanceObserver", TestObserver);
  vi.stubGlobal("window", {});
  const fetch = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
  vi.stubGlobal("fetch", fetch);
  const interval = vi.spyOn(globalThis, "setInterval");
  const clear = vi.spyOn(globalThis, "clearInterval");
  try {
    const services = createServerServices({
      id: "local",
      label: "Local",
      origin: "http://localhost",
    });
    services.api.setToken("report-session-token");
    const api: unknown = services.api;
    if (
      !api ||
      typeof api !== "object" ||
      !("fetchAuthenticatedRequest" in api) ||
      typeof api.fetchAuthenticatedRequest !== "function"
    )
      throw new Error("Reporting API missing");
    const request = api.fetchAuthenticatedRequest;
    const reportingApi = {
      async fetchAuthenticatedRequest(path: string, init: RequestInit) {
        const response: unknown = await request(path, init);
        if (!(response instanceof Response))
          throw new Error("Invalid response");
        return response;
      },
    };
    const dispose = startPerformanceReporting(reportingApi);
    expect(startPerformanceReporting(reportingApi)).toBe(dispose);
    expect(observers).toHaveLength(2);
    expect(interval).toHaveBeenCalledTimes(1);
    collector.render("composer", 5);
    dispose();
    await vi.advanceTimersByTimeAsync(0);
    expect(fetch).toHaveBeenCalledTimes(1);
    const sent = fetch.mock.calls[0]?.[0];
    expect(sent).toBeInstanceOf(Request);
    expect(sent.headers.get("X-Canvas-Token")).toBe("report-session-token");
    expect(collector.take()).toBeUndefined();
    dispose();
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(
      observers.every(
        (observer) => observer.disconnect.mock.calls.length === 1,
      ),
    ).toBe(true);
    expect(clear).toHaveBeenCalledTimes(1);
  } finally {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    vi.useRealTimers();
  }
});

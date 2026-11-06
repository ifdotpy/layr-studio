import type { PerfClientReport } from "../generated/perfReporting";

type Report = PerfClientReport;

export function createCollector() {
  let started = performance.now();
  let longTasks = { count: 0, totalMs: 0, maxMs: 0 };
  let interactions = new Map<string, Report["interactions"][number]>();
  let renders = new Map<string, Report["renders"][number]>();
  return {
    longTask(duration: number) {
      longTasks.count++;
      longTasks.totalMs += duration;
      longTasks.maxMs = Math.max(longTasks.maxMs, duration);
    },
    interaction(type: string, duration: number, inputDelay: number) {
      const row = interactions.get(type) ?? {
        type,
        count: 0,
        maxDurationMs: 0,
        maxInputDelayMs: 0,
      };
      row.count++;
      row.maxDurationMs = Math.max(row.maxDurationMs, duration);
      row.maxInputDelayMs = Math.max(row.maxInputDelayMs, inputDelay);
      interactions.set(type, row);
    },
    render(component: string, duration: number) {
      const row = renders.get(component) ?? {
        component,
        count: 0,
        totalMs: 0,
        maxMs: 0,
      };
      row.count++;
      row.totalMs += duration;
      row.maxMs = Math.max(row.maxMs, duration);
      renders.set(component, row);
    },
    take(): Report | undefined {
      const now = performance.now();
      const report = {
        windowMs: Math.round(now - started),
        longTasks,
        interactions: [...interactions.values()],
        renders: [...renders.values()],
      };
      started = now;
      longTasks = { count: 0, totalMs: 0, maxMs: 0 };
      interactions = new Map();
      renders = new Map();
      return report.longTasks.count ||
        report.interactions.length ||
        report.renders.length
        ? report
        : undefined;
    },
  };
}
export const collector = /* @__PURE__ */ createCollector();
let stopReporting: (() => void) | undefined;
export type ReportingApi = {
  fetchAuthenticatedRequest(path: string, init: RequestInit): Promise<Response>;
};

export function startPerformanceReporting(api: ReportingApi) {
  if (stopReporting) return stopReporting;
  const observers: PerformanceObserver[] = [];
  if (typeof PerformanceObserver !== "undefined") {
    if (PerformanceObserver.supportedEntryTypes?.includes("longtask")) {
      const observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries())
          collector.longTask(entry.duration);
      });
      observer.observe({ type: "longtask", buffered: true });
      observers.push(observer);
    }
    if (PerformanceObserver.supportedEntryTypes?.includes("event")) {
      const eventOptions = {
        type: "event",
        buffered: true,
        durationThreshold: 16,
      };
      const observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries())
          if (entry instanceof PerformanceEventTiming)
            collector.interaction(
              entry.name,
              entry.duration,
              entry.processingStart - entry.startTime,
            );
      });
      observer.observe(eventOptions);
      observers.push(observer);
    }
  }
  const flush = () => {
    const report = collector.take();
    if (report)
      void api
        .fetchAuthenticatedRequest("/api/perf-reporting/client", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(report),
        })
        .catch(() => {});
  };
  const timer = setInterval(flush, 60_000);
  const dispose = () => {
    if (stopReporting !== dispose) return;
    stopReporting = undefined;
    for (const observer of observers) observer.disconnect();
    clearInterval(timer);
    flush();
  };
  stopReporting = dispose;
  return dispose;
}

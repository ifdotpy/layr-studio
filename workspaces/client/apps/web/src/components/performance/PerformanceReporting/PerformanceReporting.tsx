import { useEffect } from "react";
import type { ReportingApi } from "../../../perf/collector.ts";
import { startPerformanceReporting } from "../../../perf/collector.ts";

export function PerformanceReporting({ api }: { api: ReportingApi }) {
  useEffect(() => startPerformanceReporting(api), [api]);
  return null;
}

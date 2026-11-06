import { useEffect } from "react";
import type { ReportingApi } from "./collector";
import { startPerformanceReporting } from "./collector";

export function PerformanceReporting({ api }: { api: ReportingApi }) {
  useEffect(() => startPerformanceReporting(api), [api]);
  return null;
}

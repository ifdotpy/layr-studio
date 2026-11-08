import { createAtom } from "mobx";
import type { components } from "./generated/api";

/** Maximum retained client log entries. Unit: entries. */
export const APP_LOG_MAX_ENTRIES = 200;
type Level = components["schemas"]["StudioErrorLevel"];
export type AppLogEntry = {
  id: string;
  level: Extract<Level, "log" | "error">;
  at: number; // Epoch milliseconds.
  title: string;
  reason: string;
  detail?: string;
  dedupeKey?: string;
};
let entries: AppLogEntry[] = [];
let nextId = 0;
const atom = createAtom("appLog");
export const appLogEntries = () => {
  atom.reportObserved();
  return entries;
};
export function appendAppLog(
  entry: Omit<AppLogEntry, "id" | "level"> & { level?: AppLogEntry["level"] },
) {
  if (
    entry.dedupeKey &&
    entries.some((item) => item.dedupeKey === entry.dedupeKey)
  )
    return;
  entries = [
    ...entries,
    { ...entry, id: `app-log:${++nextId}`, level: entry.level ?? "log" },
  ].slice(-APP_LOG_MAX_ENTRIES);
  atom.reportChanged();
}
export function clearAppLog() {
  entries = [];
  atom.reportChanged();
}

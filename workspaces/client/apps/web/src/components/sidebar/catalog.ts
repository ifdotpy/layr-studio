import type { Agent } from "../../types";

/** Select the most recent activity, with ascending ID for timestamp ties. */
export function mostRecentActivity<
  T extends Pick<Agent, "id" | "updated" | "created">,
>(rows: T[]) {
  let selected: T | undefined;
  for (const row of rows) {
    if (!selected) {
      selected = row;
      continue;
    }
    const updated = row.updated ?? row.created ?? 0;
    const selectedUpdated = selected.updated ?? selected.created ?? 0;
    if (
      updated > selectedUpdated ||
      (updated === selectedUpdated && row.id < selected.id)
    )
      selected = row;
  }
  return selected;
}

export function itemGroup(path: string, parent: string | null = null) {
  return JSON.stringify(["items", path, parent]);
}

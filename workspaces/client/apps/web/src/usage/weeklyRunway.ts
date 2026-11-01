import type { components } from "../generated/api";

type WeeklyRunway = components["schemas"]["WeeklyRunway"];

/** Select only the server-supplied state for the current instant. */
export function weeklyRunway(
  view: WeeklyRunway | null | undefined,
  now: number,
) {
  if (!view) return null;
  return view.validUntil !== null && now >= view.validUntil
    ? view.next
    : view.current;
}

import { RunwayColor } from "../generated/api";
import { expect, it } from "vitest";
import type { components } from "../generated/api";
import { weeklyRunway } from "./weeklyRunway";

it("uses the server's next state at the deadline without assuming a reset", () => {
  const view = {
    computedAt: 100,
    validUntil: 110,
    current: { color: RunwayColor.red, days: 0, unlimited: false, windows: [] },
    next: {
      color: RunwayColor.gray,
      days: null,
      unlimited: false,
      windows: [],
    },
  } satisfies components["schemas"]["WeeklyRunway"];
  expect(weeklyRunway(view, 109)).toBe(view.current);
  expect(weeklyRunway(view, 110)).toBe(view.next);
  expect(weeklyRunway(null, 110)).toBeNull();
});

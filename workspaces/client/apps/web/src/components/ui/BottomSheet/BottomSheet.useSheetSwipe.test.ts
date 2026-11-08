import { expect, it } from "vitest";
import { swipeCloses } from "./useSheetSwipe.ts";

it("closes past the distance or on a flick, and springs back otherwise", () => {
  expect(swipeCloses(120, 800)).toBe(true);
  expect(swipeCloses(40, 50)).toBe(true);
  expect(swipeCloses(40, 400)).toBe(false);
  expect(swipeCloses(0, 10)).toBe(false);
});

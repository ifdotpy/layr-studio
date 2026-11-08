import { expect, it } from "vitest";
import { codePointsExceed } from "./commands.ts";

it("counts code points, not UTF-16 units, near the limit", () => {
  expect(codePointsExceed("abcd", 4)).toBe(false);
  expect(codePointsExceed("abcde", 4)).toBe(true);
  // Three emoji are six UTF-16 units but three code points.
  expect(codePointsExceed("😀😀😀", 4)).toBe(false);
  expect(codePointsExceed("😀😀😀😀😀", 4)).toBe(true);
  expect(codePointsExceed("", 0)).toBe(false);
});

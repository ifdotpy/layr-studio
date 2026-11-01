import { expect, test } from "vitest";
import { LimitColor } from "../generated/api";
import { remainingLimitColor } from "./accountLimitColor";

test.each(Object.values(LimitColor))(
  "uses the server-selected %s color",
  (color) => {
    expect(remainingLimitColor(color)).toBe(`var(--studio-state-${color})`);
  },
);

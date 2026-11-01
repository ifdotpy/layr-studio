import { LimitColor } from "../generated/api";
import type { LimitColor as LimitColorValue } from "../generated/api";

const colors = {
  [LimitColor.paused]: "var(--studio-state-paused)",
  [LimitColor.unread]: "var(--studio-state-unread)",
  [LimitColor.answer]: "var(--studio-state-answer)",
  [LimitColor.error]: "var(--studio-state-error)",
} satisfies Record<LimitColorValue, string>;

export const remainingLimitColor = (color: LimitColorValue) => colors[color];

import { LimitColor, RunwayColor, type components } from "../generated/api";

export function projectedLimits(
  accountKey: string,
  at: number,
  validUntil: number | null = 250,
): components["schemas"]["RateLimitView"] {
  const unknown = {
    buckets: [],
    remaining: null,
    color: LimitColor.paused,
    low: false,
  };
  const runway = {
    color: RunwayColor.gray,
    days: null,
    unlimited: false,
    windows: [],
  };
  return {
    observation: { accountKey, at, data: { accountId: null } },
    projection: {
      agents: {},
      status: { kind: "current" },
      runway: {
        computedAt: at,
        validUntil: null,
        current: runway,
        next: runway,
      },
      reset: {
        present: false,
        accountId: null,
        availableCount: null,
        countLabel: "Count unavailable",
        emptyMessage: "Reset credit details unavailable. Refresh to check.",
        credits: [],
      },
      buckets: {
        current: { ...unknown, remaining: 33, color: LimitColor.answer },
        next: unknown,
        validUntil,
      },
    },
  };
}

import type { components } from "../../../generated/api.ts";

import { remainingLimitColor } from "../usage/support/accountLimitColor.ts";

export function AccountRemainingBar({
  remaining,
  color,
  label = "Remaining limit",
}: {
  remaining: number | null;
  color: components["schemas"]["LimitColor"];
  label?: string;
}) {
  return (
    <span
      className="setup-limit"
      role={remaining === null ? "img" : "meter"}
      aria-label={remaining === null ? `${label}: Unavailable` : label}
      aria-valuemin={remaining === null ? undefined : 0}
      aria-valuemax={remaining === null ? undefined : 100}
      aria-valuenow={remaining ?? undefined}
    >
      <span
        style={{
          width: `${remaining ?? 0}%`,
          background: remainingLimitColor(color),
        }}
      />
    </span>
  );
}

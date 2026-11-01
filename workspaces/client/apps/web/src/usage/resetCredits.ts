import type { components } from "../generated/api";

type Credit = Pick<
  components["schemas"]["ResetCreditView"],
  "enabled" | "validUntil" | "enabledAfter"
>;

/** Select the server's credit state at its supplied expiry. */
export function resetCreditEnabled(credit: Credit, now: number): boolean {
  return credit.validUntil !== null && now >= credit.validUntil
    ? credit.enabledAfter
    : credit.enabled;
}

import { expect, it } from "vitest";
import { resetCreditEnabled } from "./resetCredits";

it("uses the server's post-expiry reset state at the exact deadline", () => {
  const credit = {
    id: "credit",
    enabled: true,
    validUntil: 110,
    enabledAfter: false,
  };
  expect(resetCreditEnabled(credit, 109)).toBe(true);
  expect(resetCreditEnabled(credit, 110)).toBe(false);
});

import { describe, expect, it } from "vitest";
import {
  accountDisplayName,
  accountTooltip,
  accountCanStart,
} from "./accountName";

describe("account names", () => {
  it("uses server eligibility even when status disagrees", () => {
    const account = {
      id: "a",
      home: "/a",
      label: "A",
      source: "test",
      provider: "codex" as const,
      status: "ready" as const,
      eligibility: { kind: "unavailable" as const, reason: "Unavailable" },
    };
    expect(accountCanStart(account)).toBe(false);
  });
  it("allows a pinned native proof attempt without claiming authentication", () => {
    const account = {
      id: "claude-local",
      home: "/profile",
      label: "Work",
      source: "Claude",
      provider: "claude" as const,
      status: "error" as const,
      canAttemptNativeProof: true,
      eligibility: { kind: "available" as const },
    };
    expect(accountCanStart(account)).toBe(true);
    expect(account.status).toBe("error");
    expect(accountCanStart(undefined)).toBe(false);
  });
  it("uses the persisted label and the full email tooltip", () => {
    const account = { label: " Work 1 ", email: "work@example.com" };
    expect(accountDisplayName(account)).toBe("Work 1");
    expect(accountTooltip(account)).toBe("work@example.com");
  });
  it("uses the email prefix after the label is cleared", () => {
    expect(accountDisplayName({ label: "", email: "work@example.com" })).toBe(
      "work",
    );
    expect(accountDisplayName({ label: "  ", email: "work@example.com" })).toBe(
      "work",
    );
    expect(accountDisplayName({ label: "", email: null })).toBe("Account");
    expect(accountTooltip({ email: null })).toBe("");
  });
  it("shows only the part before @ when the label is an email", () => {
    expect(
      accountDisplayName({
        label: "ifdotpy@gmail.com",
        email: "ifdotpy@gmail.com",
      }),
    ).toBe("ifdotpy");
  });
});

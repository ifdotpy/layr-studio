import { describe, expect, it } from "vitest";
import type { Agent } from "../types";
import type { UsageAccount } from "./Usage";
import { roleAccounts } from "./roleAccounts";

describe("role accounts", () => {
  it("shows only the chat lead's account", () => {
    const accounts = [{ key: "default" }, { key: "chosen" }] as UsageAccount[];
    const lead = { accountKey: "chosen", concurrency: 3 } as Agent;
    expect(roleAccounts(lead, accounts)).toEqual([
      { role: "Lead", account: accounts[1], automatic: false },
    ]);
  });

  it("does not invent an account when the lead account is absent", () => {
    expect(
      roleAccounts(
        { accountKey: "missing" } as Agent,
        [{ key: "default" }] as UsageAccount[],
      ),
    ).toEqual([]);
  });
});

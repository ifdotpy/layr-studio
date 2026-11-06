import type { Agent } from "../types";
import type { UsageAccount } from "./Usage";

export function roleAccounts(lead: Agent, accounts: UsageAccount[]) {
  const account = accounts.find(
    (item) => item.key === (lead.accountKey || "default"),
  );
  return account ? [{ role: "Lead", account, automatic: false }] : [];
}

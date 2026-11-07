import type { Agent } from "../types";
import type { UsageAccount } from "./Usage";
import { modelProvider } from "./accountPreference";
import { subagentsOff } from "./agents/SubagentConcurrencyControl";

export type Role = "Lead" | "Workers" | "Review";
export type RoleAccount = {
  role: Role;
  account: UsageAccount;
  /** The server picks this account when the role starts; the dot shows a likely one. */
  automatic: boolean;
};

// Runway colors from the most to the least remaining allowance.
const colorRank = ["green", "yellow", "red", "gray"];
const rank = (account: UsageAccount) => {
  const current = account.runway?.current;
  const color = colorRank.indexOf(current?.color ?? "gray");
  const days = current?.unlimited ? Infinity : (current?.days ?? -1);
  return { color: color < 0 ? colorRank.length : color, days };
};
function freest(accounts: UsageAccount[]) {
  return accounts.reduce((best, account) => {
    const a = rank(account);
    const b = rank(best);
    return a.color < b.color || (a.color === b.color && a.days > b.days)
      ? account
      : best;
  });
}

/**
 * The account of each role of a chat, at most one per role: Lead, Workers and
 * Review. With subagents off only the lead works. Workers take an account the
 * chat does not exclude, for their model's provider, when they start; the dot
 * shows the one with the most allowance. A reviewer the lead starts keeps the
 * lead's account when the chat allows it and it serves the review model.
 */
export function roleAccounts(
  lead: Agent,
  accounts: UsageAccount[],
): RoleAccount[] {
  const leadAccount = accounts.find(
    (item) => item.key === (lead.accountKey || "default"),
  );
  if (!leadAccount) return [];
  const roles: RoleAccount[] = [
    { role: "Lead", account: leadAccount, automatic: false },
  ];
  if (subagentsOff(lead)) return roles;
  const excluded = new Set(lead.excludedAccountKeys ?? []);
  const allowed = accounts.filter((account) => !excluded.has(account.key));
  const serves = (account: UsageAccount, model: string | null | undefined) => {
    const provider = model ? modelProvider(model) : null;
    return !provider || account.provider === provider;
  };
  const automatic = (role: Role, model: string | null | undefined) => {
    const eligible = allowed.filter((account) => serves(account, model));
    return eligible.length
      ? { role, account: freest(eligible), automatic: true }
      : undefined;
  };
  const workerModel = lead.workerDefaults?.model;
  // A review without its own model runs on the model of its caller, the lead.
  const reviewModel = lead.reviewDefaults?.model || lead.model;
  const workers = automatic("Workers", workerModel);
  const review =
    !excluded.has(leadAccount.key) && serves(leadAccount, reviewModel)
      ? { role: "Review" as const, account: leadAccount, automatic: false }
      : automatic("Review", reviewModel);
  return [...roles, ...(workers ? [workers] : []), ...(review ? [review] : [])];
}

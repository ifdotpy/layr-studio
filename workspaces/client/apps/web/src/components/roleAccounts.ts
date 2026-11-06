import type { Agent } from "../types";
import type { UsageAccount } from "./Usage";
import { modelProvider } from "./accountPreference";
import { subagentsOff } from "./agents/SubagentConcurrencyControl";

export type Role = "Lead" | "Workers" | "Review";
export type RoleAccount = {
  role: Role;
  account: UsageAccount;
  /** No account is chosen: subagents take a free account at start. */
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
/** The account with the most remaining allowance, as automatic choice shows it. */
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
 * Review. With subagents off only the Lead works. Review tasks run with the
 * Workers account unless Review names its own. An automatic role shows the
 * account with the most allowance of its model's provider.
 */
export function roleAccounts(
  lead: Agent,
  accounts: UsageAccount[],
): RoleAccount[] {
  const fallback = accounts[0];
  if (!fallback) return [];
  const byKey = (key: string | null | undefined) =>
    key ? accounts.find((account) => account.key === key) : undefined;
  const leadAccount = byKey(lead.accountKey || "default") ?? fallback;
  const roles: RoleAccount[] = [
    { role: "Lead", account: leadAccount, automatic: false },
  ];
  if (subagentsOff(lead)) return roles;
  const automatic = (model: string | null | undefined) => {
    const provider = model ? modelProvider(model) : null;
    const eligible = accounts.filter(
      (account) => !provider || account.provider === provider,
    );
    return freest(eligible.length ? eligible : accounts);
  };
  const workerKey = lead.workerDefaults?.accountKey;
  const workers: RoleAccount = byKey(workerKey)
    ? { role: "Workers", account: byKey(workerKey)!, automatic: false }
    : {
        role: "Workers",
        account: automatic(lead.workerDefaults?.model),
        automatic: true,
      };
  const reviewKey = lead.reviewDefaults?.accountKey;
  const review: RoleAccount = byKey(reviewKey)
    ? { role: "Review", account: byKey(reviewKey)!, automatic: false }
    : { ...workers, role: "Review" };
  return [...roles, workers, review];
}

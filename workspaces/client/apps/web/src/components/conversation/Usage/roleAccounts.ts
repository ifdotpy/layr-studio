import type { Agent } from "../../../types.ts";
import type { DeepReadonly } from "../../../deepReadonly.ts";
import type { components } from "../../../generated/api.ts";
type Runway = DeepReadonly<components["schemas"]["WeeklyRunway"]> | undefined;
import { modelProvider } from "../../accounts/support/accountPreference.ts";
import { subagentsOff } from "../../agents/SubagentConcurrencyControl/SubagentConcurrencyControl.tsx";

export type Role = "Lead" | "Workers" | "Review";
export type RoleAccount<Account> = {
  role: Role;
  account: Account;
  /** The server picks this account when the role starts; the dot shows a likely one. */
  automatic: boolean;
};

// Runway colors from the most to the least remaining allowance.
const colorRank = ["green", "yellow", "red", "gray"];
const rank = (runway: Runway) => {
  const current = runway?.current;
  const color = colorRank.indexOf(current?.color ?? "gray");
  const days = current?.unlimited ? Infinity : (current?.days ?? -1);
  return { color: color < 0 ? colorRank.length : color, days };
};
function freest<Account>(
  accounts: Account[],
  runway: (account: Account) => Runway,
) {
  return accounts.reduce((best, account) => {
    const a = rank(runway(account));
    const b = rank(runway(best));
    return a.color < b.color || (a.color === b.color && a.days > b.days)
      ? account
      : best;
  });
}

/**
 * The account of each role of a chat, at most one per role: Lead, Workers and
 * Review. With subagents off only the lead works. Workers take an account the
 * chat does not exclude for workers, for their model's provider, when they
 * start; the dot shows the one with the most allowance. A reviewer the lead
 * starts keeps the lead's account when the chat allows it for reviewers and it
 * serves the review model.
 */
export function accountsForRoles<
  Account extends { readonly provider?: string | undefined },
>(
  lead: Agent,
  accounts: readonly Account[],
  detail: Omit<Agent, "id">,
  key: (account: Account) => string,
  runway: (account: Account) => Runway,
): RoleAccount<Account>[] {
  const leadAccount = accounts.find(
    (item) => key(item) === (lead.accountKey || "default"),
  );
  if (!leadAccount) return [];
  const roles: RoleAccount<Account>[] = [
    { role: "Lead", account: leadAccount, automatic: false },
  ];
  if (subagentsOff(detail)) return roles;
  const serves = (account: Account, model: string | null | undefined) => {
    const provider = model ? modelProvider(model) : null;
    return !provider || account.provider === provider;
  };
  const automatic = (
    role: Role,
    model: string | null | undefined,
    excluded: Set<string>,
  ) => {
    const eligible = accounts.filter(
      (account) => !excluded.has(key(account)) && serves(account, model),
    );
    return eligible.length
      ? { role, account: freest(eligible, runway), automatic: true }
      : undefined;
  };
  const workers = automatic(
    "Workers",
    detail.workerDefaults?.model,
    new Set(detail.workerExcludedAccountKeys ?? []),
  );
  // A review without its own model runs on the model of its caller, the lead.
  const reviewModel = detail.reviewDefaults?.model || lead.model;
  const reviewExcluded = new Set(detail.reviewerExcludedAccountKeys ?? []);
  const review =
    !reviewExcluded.has(key(leadAccount)) && serves(leadAccount, reviewModel)
      ? { role: "Review" as const, account: leadAccount, automatic: false }
      : automatic("Review", reviewModel, reviewExcluded);
  return [...roles, ...(workers ? [workers] : []), ...(review ? [review] : [])];
}

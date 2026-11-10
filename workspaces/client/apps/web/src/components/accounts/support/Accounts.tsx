import type { StateCollections } from "../../../generated/observableResources.ts";

export type AccountsState = NonNullable<
  ReturnType<StateCollections["accountsSummary"]["get"]>
>;

export type Account = AccountsState["accounts"][number];

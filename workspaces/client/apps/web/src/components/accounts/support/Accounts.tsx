import type { GetResult } from "../../../api.ts";

export type AccountsState = import("../../../deepReadonly.ts").DeepReadonly<
  GetResult<"/api/accounts">
>;

export type Account = AccountsState["accounts"][number];

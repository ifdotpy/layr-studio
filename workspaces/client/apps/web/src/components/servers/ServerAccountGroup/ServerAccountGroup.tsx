import type { AccountSource } from "../../../servers/ServerAccountLimits.tsx";
import type { ReactNode } from "react";
import AccountGroup from "../../accounts/AccountGroup/AccountGroup.tsx";
import type { Account } from "../../accounts/support/Accounts.tsx";
import { ServerServicesProvider } from "../ServerServicesProvider/ServerServicesProvider.tsx";
import type { ServerServices } from "../../../servers/serverServices.ts";

/** Account groups are settings of one Studio server,
 * stored on that server's account, so the request goes to that server. */
export function ServerAccountGroup({
  source,
  onSaved,
}: {
  source: AccountSource;
  onSaved: (accounts: readonly Account[]) => void;
}) {
  return (
    <ServerScope services={source.services}>
      <AccountGroup
        account={source.account}
        serverLabel={source.serverLabel}
        onSaved={(state) => onSaved(state.accounts)}
      />
    </ServerScope>
  );
}

/** Requests in this subtree go to the given server, or to the default API. */
function ServerScope({
  services,
  children,
}: {
  services?: ServerServices | undefined;
  children: ReactNode;
}) {
  return services ? (
    <ServerServicesProvider services={services}>
      {children}
    </ServerServicesProvider>
  ) : (
    children
  );
}

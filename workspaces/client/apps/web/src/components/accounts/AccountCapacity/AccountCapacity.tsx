import type { Account } from "../support/Accounts.tsx";
import { AccountCapacityView } from "../AccountCapacityView/AccountCapacityView.tsx";
import { observer } from "mobx-react-lite";
import { AccountStatus as GeneratedApiAccountStatus } from "../../../generated/api.ts";
import { useAccountLimitView } from "../support/AccountTiles.tsx";
import type { ReactNode } from "react";

export const AccountCapacity = observer(function AccountCapacity({
  account,
  card = false,
  extraDetails,
  facts,
}: {
  account: Account;
  card?: boolean;
  extraDetails?: ReactNode;
  facts?: ReactNode;
}) {
  const view = useAccountLimitView(account);
  if (account.status !== GeneratedApiAccountStatus.ready && !card) return null;
  return (
    <AccountCapacityView
      account={account}
      view={view}
      card={card}
      extraDetails={extraDetails}
      facts={facts}
    />
  );
});

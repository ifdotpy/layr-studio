import {
  type LimitAccount,
  useAccountLimitView,
  remainingText,
} from "../support/AccountTiles.tsx";
import { observer } from "mobx-react-lite";

/** The remaining limit of an account as short text, for account lists. */
export const AccountRemainingText = observer(function AccountRemainingText({
  account,
}: {
  account: LimitAccount;
}) {
  const { remaining } = useAccountLimitView(account);
  return remaining === null ? null : (
    <small className="account-remaining" data-exhausted={remaining === 0}>
      {remainingText(remaining)}
    </small>
  );
});

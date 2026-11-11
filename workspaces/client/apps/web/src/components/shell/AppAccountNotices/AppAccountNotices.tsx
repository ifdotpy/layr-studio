import AccountSignInNotice from "../../accounts/AccountSignInNotice/AccountSignInNotice.tsx";
import { AgentStatus } from "../../../generated/api.ts";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const AppAccountNotices = observer(function AppAccountNotices() {
  const a = useServerAreas(),
    catalog = a.ui.catalog;
  const agent = a.navigation.selectedAgent,
    lead = a.navigation.selectedLead || agent;
  const key =
    agent?.accountKey ||
    lead?.accountKey ||
    (agent ? "default" : a.accounts?.defaultAccountKey) ||
    "default";
  return (
    <>
      {[
        ...(a.accounts?.accounts || []),
        ...(a.accounts?.archivedAccounts || []),
      ]
        .filter(
          (account) =>
            account.id === key ||
            catalog.agents.some(
              (member) =>
                member.rootId === lead?.id &&
                (member.accountKey || "default") === account.id &&
                member.status === AgentStatus.failed,
            ),
        )
        .map((account) => (
          <AccountSignInNotice
            key={account.id}
            account={account}
            errors={catalog.errors(account.id, agent?.id, lead?.id)}
            onSignIn={a.ui.signIn}
          />
        ))}
    </>
  );
});

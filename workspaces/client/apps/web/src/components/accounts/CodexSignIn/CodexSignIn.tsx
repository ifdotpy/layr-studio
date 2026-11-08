import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Modal } from "@mantine/core";
import type { Account } from "../support/Accounts.tsx";
import type { AccountsController } from "../Accounts/Accounts.tsx";
import AccountSignIn from "../AccountSignIn/AccountSignIn.tsx";
import "../../../api.ts";

export default function CodexSignIn({
  account,
  state,
  onClose,
  serverLabel,
}: {
  account: Account;
  state: AccountsController;
  onClose: () => void;
  serverLabel?: string | undefined;
}) {
  const { post } = useServerApi();

  return (
    <Modal
      opened
      onClose={onClose}
      title={`Sign in to Codex · ${account.email || account.label}${serverLabel ? ` · ${serverLabel}` : ""}`}
    >
      <AccountSignIn
        key={`${state.scope}:${account.id}`}
        state={state}
        targetAccount={account}
        onConnected={async (connected) => {
          if (connected.disconnected)
            state.setData(
              await post("/api/accounts/reconnect", {
                account_key: connected.id,
              }),
            );
        }}
      />
    </Modal>
  );
}

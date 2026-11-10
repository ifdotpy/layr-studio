import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Modal } from "@mantine/core";
import type { Account } from "../support/Accounts.tsx";
import AccountSignIn from "../AccountSignIn/AccountSignIn.tsx";
import "../../../api.ts";

export default function CodexSignIn({
  account,
  scope,
  onClose,
  serverLabel,
}: {
  account: Account;
  scope?: string | undefined;
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
        key={`${scope}:${account.id}`}
        scope={scope}
        targetAccount={account}
        onConnected={async (connected) => {
          if (connected.disconnected)
            await post("/api/accounts/reconnect", {
              account_key: connected.id,
            });
        }}
      />
    </Modal>
  );
}

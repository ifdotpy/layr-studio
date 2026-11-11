import { Modal } from "@mantine/core";
import type { Account } from "../support/Accounts.tsx";
import AccountSignIn from "../AccountSignIn/AccountSignIn.tsx";

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
      />
    </Modal>
  );
}

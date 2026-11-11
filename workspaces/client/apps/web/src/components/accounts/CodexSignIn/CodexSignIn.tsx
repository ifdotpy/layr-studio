import { Modal, Stack, TextInput } from "@mantine/core";
import type { ReactNode } from "react";
import type { Account } from "../support/Accounts.tsx";
import AccountSignIn from "../AccountSignIn/AccountSignIn.tsx";
import { layerZIndex } from "../../../theme.ts";

export type SignInProps = {
  account?: Account | undefined;
  scope?: string | undefined;
  label?: string;
  email?: string;
  serverLabel?: string | undefined;
  opened?: boolean;
  onClose: () => void;
  onReady?: () => unknown;
};

/** Common frame for new and saved subscription sign-ins on either provider. */
export default function CodexSignIn({
  provider = "Codex",
  email,
  scope,
  label,
  account,
  serverLabel,
  opened = true,
  onClose,
  children,
}: SignInProps & { provider?: string; label?: string; children?: ReactNode }) {
  return (
    <Modal
      opened={opened}
      onClose={onClose}
      centered
      zIndex={layerZIndex.modalAbovePopover}
      title={`Sign in to ${provider} · ${label || account?.email || account?.label || provider}${account && serverLabel ? ` · ${serverLabel}` : ""}`}
    >
      <Stack gap="sm">
        {serverLabel &&
          (account ? (
            <p>Server: {serverLabel}</p>
          ) : (
            <TextInput label="Server" value={serverLabel} readOnly />
          ))}
        {provider === "Codex" && !account && (
          <p>
            Open auth.openai.com/codex/device. Use a private window if your
            browser has another ChatGPT account signed in.
          </p>
        )}
        {children || (
          <AccountSignIn
            key={`${scope}:${account?.id}`}
            scope={scope}
            targetAccount={account}
            label={label}
            emailHint={email}
          />
        )}
      </Stack>
    </Modal>
  );
}

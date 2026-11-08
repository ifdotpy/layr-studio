import { LOGIN_LABEL_MAX_CHARS } from "../../../generated/sharedValues.ts";
import { Button, Stack, TextInput } from "@mantine/core";
import { createContext, useContext, type ReactNode } from "react";
import { observable, runInAction } from "mobx";
import { observer } from "mobx-react-lite";
import {
  PROVIDER_KINDS,
  PROVIDER_LABELS,
  ProviderKind,
} from "../../../generated/providerKind.ts";

export const ACCOUNT_SIGN_IN_HINTS: Record<ProviderKind, string> = {
  [ProviderKind.Codex]: "ChatGPT subscription, one-time code",
  [ProviderKind.Claude]: "Claude subscription, link + paste code",
};

export type AccountAddDraft = {
  provider: ProviderKind | "";
  label: string;
  email: string;
};
export const createAccountAddDraft = () =>
  observable<AccountAddDraft>({ provider: "", label: "", email: "" });
export const AccountAddActionsContext = createContext<{
  onCancel: () => void;
  onContinue: () => void;
  unavailable?: boolean;
} | null>(null);

export const AccountAddForm = observer(function AccountAddForm({
  model,
  children,
  gap = "md",
}: {
  model: AccountAddDraft;
  children: ReactNode;
  gap?: "sm" | "md";
}) {
  const actions = useContext(AccountAddActionsContext);
  if (!actions) throw new Error("Account add actions are required");
  return (
    <Stack gap={gap}>
      <div
        className="account-provider-choices"
        role="group"
        aria-label="Account type"
      >
        {PROVIDER_KINDS.map((kind) => (
          <button
            key={kind}
            type="button"
            className={`account-provider-choice${model.provider === kind ? " selected" : ""}`}
            aria-pressed={model.provider === kind}
            onClick={() =>
              runInAction(() => {
                model.provider = kind;
              })
            }
          >
            <strong>{PROVIDER_LABELS[kind]}</strong>
            <span>{ACCOUNT_SIGN_IN_HINTS[kind]}</span>
          </button>
        ))}
      </div>
      {children}
      <TextInput
        label="Name"
        value={model.label}
        maxLength={LOGIN_LABEL_MAX_CHARS}
        onChange={(event) =>
          runInAction(() => {
            model.label = event.currentTarget.value;
          })
        }
      />
      <TextInput
        label="Email (optional)"
        type="email"
        value={model.email}
        onChange={(event) =>
          runInAction(() => {
            model.email = event.currentTarget.value;
          })
        }
        description="Studio checks that you sign in with this account."
      />
      <div className="account-add-dialog-actions">
        <Button variant="default" onClick={actions.onCancel}>
          Cancel
        </Button>
        <Button
          disabled={!model.provider || !!actions.unavailable}
          onClick={actions.onContinue}
        >
          Continue
        </Button>
      </div>
    </Stack>
  );
});

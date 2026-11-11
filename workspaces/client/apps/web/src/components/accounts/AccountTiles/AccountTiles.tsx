import { AccountTilesLayout } from "../AccountTilesLayout/AccountTilesLayout.tsx";
import {
  accountLimitView,
  useAccountLimitView,
} from "../support/AccountTiles.tsx";
import { AccountTileLabel } from "../AccountTileLabel/AccountTileLabel.tsx";
import { AccountRemainingBar } from "../AccountRemainingBar/AccountRemainingBar.tsx";
import { observer } from "mobx-react-lite";
import { Checkbox, Tooltip } from "@mantine/core";
import { Check } from "lucide-react";
import { useState } from "react";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import { accountDisplayName, accountTooltip } from "../../../accountName.ts";
import type { Account } from "../support/Accounts.tsx";
import type { SetupRole } from "../../agents/AgentSetupPicker/AgentSetupPicker.tsx";

export const AccountTiles = observer(function AccountTiles(
  p: AccountTilesProps,
) {
  const excluded = p.excludedKeys;
  const onExcludedChange = p.onExcludedKeysChange;
  const selected = p.multiple ? p.value[0] : p.value;
  const selectedProvider =
    p.accounts.find((account) => account.id === selected)?.provider || "";
  const selection = p.multiple
    ? selectedProvider
    : JSON.stringify([p.value, selectedProvider]);
  const [choice, setChoice] = useState([selection, selectedProvider]);
  if (choice[0] !== selection) setChoice([selection, selectedProvider]);
  const provider = p.provider || choice[1] || "";
  const providers = p.providers || [
    ...new Set(
      p.accounts
        .filter((account) => !account.disconnected && !account.deleted)
        .map((account) => account.provider),
    ),
  ];
  const select = (key: string) => {
    if (p.multiple)
      p.onChange(
        p.value.includes(key)
          ? p.value.filter((value) => value !== key)
          : [...p.value, key],
      );
    else p.onChange(key);
  };
  const accounts = p.accounts.filter(
    (account) => account.provider === provider,
  );
  const allowedCount = p.accounts.filter(
    (account) => !excluded?.includes(account.id),
  ).length;
  const role = p.exclusionRole === "worker" ? "workers" : "reviewers";
  return (
    <AccountTilesLayout
      label={p.label}
      provider={provider}
      providers={providers}
      data-value={p.multiple ? p.value.join(",") : p.value}
      disabled={p.disabled}
      heading={
        p.showLabel !== false && (
          <span className="account-tiles-label">
            {p.visibleLabel || p.label}
          </span>
        )
      }
      showSingleProvider
      onProvider={(value) => {
        setChoice([selection, value]);
        p.onProvider?.(value);
      }}
    >
      <div className="setup-accounts">
        {accounts.map((account) => (
          <div
            key={account.id}
            className={excluded ? "setup-account-choice" : undefined}
          >
            {excluded && onExcludedChange && p.exclusionRole && (
              <Tooltip
                label={`Use for ${role}`}
                events={{ hover: true, focus: true, touch: false }}
              >
                <Checkbox
                  className="setup-account-exclusion"
                  aria-label={`Use ${setupAccountName(account)} for ${role}`}
                  checked={!excluded.includes(account.id)}
                  disabled={
                    p.disabled ||
                    (allowedCount === 1 && !excluded.includes(account.id))
                  }
                  onChange={(event) =>
                    onExcludedChange(
                      event.currentTarget.checked
                        ? excluded.filter((key) => key !== account.id)
                        : [...excluded, account.id],
                    )
                  }
                />
              </Tooltip>
            )}
            <AccountTile
              account={account}
              selected={
                p.multiple
                  ? p.value.includes(account.id)
                  : p.value === account.id
              }
              multiple={p.multiple}
              readOnly={
                p.selectable === false ||
                (!p.multiple &&
                  p.value === account.id &&
                  !excluded &&
                  accounts.length === 1)
              }
              disabled={
                !!p.disabled ||
                !!p.accountDisabled?.(account) ||
                !!p.accountUnavailableReason?.(account)
              }
              onSelect={() => select(account.id)}
            />
            {p.accountUnavailableReason?.(account) && (
              <small>{p.accountUnavailableReason(account)}</small>
            )}
          </div>
        ))}
      </div>
    </AccountTilesLayout>
  );
});

export function setupAccountName(account?: Account) {
  return account ? accountDisplayName(account) : "";
}

export type AccountLimitView = ReturnType<typeof accountLimitView>;

const AccountTile = observer(function AccountTile({
  account,
  selected,
  disabled,
  onSelect,
  multiple = false,
  readOnly = false,
}: {
  account: Account;
  selected: boolean;
  disabled: boolean;
  onSelect: () => void;
  multiple?: boolean | undefined;
  readOnly?: boolean;
}) {
  const { remaining, color } = useAccountLimitView(account, {
    signedInOnly: false,
  });
  const content = (
    <>
      <AccountTileLabel name={setupAccountName(account)} remaining={remaining}>
        {!readOnly && multiple && selected && (
          <Check size={13} aria-hidden="true" />
        )}
      </AccountTileLabel>
      {remaining !== null && (
        <AccountRemainingBar remaining={remaining} color={color} />
      )}
    </>
  );
  const Control = readOnly ? "div" : ActionButton;
  return (
    <Tooltip label={accountTooltip(account)}>
      <Control
        className={`setup-account${readOnly ? " setup-account-status" : ""}`}
        data-account-key={account.id}
        data-selected={selected || undefined}
        {...(!readOnly && {
          "data-exhausted": remaining === 0 || undefined,
          "aria-label": `Select account ${setupAccountName(account)}`,
          "aria-description":
            remaining === null ? undefined : `${remaining}% remaining`,
          "aria-pressed": selected,
          disabled,
          onClick: onSelect,
        })}
      >
        {content}
      </Control>
    </Tooltip>
  );
});

type AccountTilesProps = {
  label: string;
  showLabel?: boolean;
  selectable?: boolean;
  visibleLabel?: string;
  accounts: Account[];
  disabled?: boolean | undefined;
  provider?: string;
  providers?: string[];
  onProvider?: (provider: string) => void;
  accountDisabled?: ((account: Account) => boolean) | undefined;
  accountUnavailableReason?:
    | ((account: Account) => string | undefined)
    | undefined;
  excludedKeys?: readonly string[] | undefined;
  onExcludedKeysChange?: ((keys: string[]) => void) | undefined;
  exclusionRole?: Exclude<SetupRole, "orchestrator"> | undefined;
} & (
  | { multiple?: false; value: string; onChange: (value: string) => void }
  | { multiple: true; value: string[]; onChange: (value: string[]) => void }
);

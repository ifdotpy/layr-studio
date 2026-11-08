import { ProviderMark } from "../ProviderMark/ProviderMark.tsx";
import {
  accountLimitView,
  useAccountLimitView,
} from "../support/AccountTiles.tsx";
import { AccountTileLabel } from "../AccountTileLabel/AccountTileLabel.tsx";
import { AccountRemainingBar } from "../AccountRemainingBar/AccountRemainingBar.tsx";
import { observer } from "mobx-react-lite";
import { Checkbox, SegmentedControl, Tooltip } from "@mantine/core";
import { Check } from "lucide-react";
import { useEffect, useState } from "react";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import { accountDisplayName, accountTooltip } from "../../../accountName.ts";
import type { Account } from "../support/Accounts.tsx";
import type { SetupRole } from "../../agents/AgentSetupPicker/AgentSetupPicker.tsx";

import { providerLabel } from "../../../generated/providerKind.ts";

export function AccountTiles(p: AccountTilesProps) {
  const excluded = p.excludedKeys;
  const onExcludedChange = p.onExcludedKeysChange;
  const selected = p.multiple ? p.value[0] : p.value;
  const selectedProvider =
    p.accounts.find((account) => account.id === selected)?.provider || "";
  const [localProvider, setLocalProvider] = useState<string>(selectedProvider);
  const provider = p.provider || localProvider;
  useEffect(() => {
    setLocalProvider(selectedProvider);
  }, [selectedProvider]);
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
  return (
    <div
      className="account-tiles"
      role="group"
      aria-label={p.label}
      data-value={p.multiple ? p.value.join(",") : p.value}
      aria-disabled={p.disabled}
    >
      {p.showLabel !== false && (
        <span className="account-tiles-label">{p.visibleLabel || p.label}</span>
      )}
      {providers.length > 0 && (
        <SegmentedControl
          fullWidth
          aria-label={`${p.label} provider`}
          value={provider}
          disabled={!!p.disabled}
          onChange={(value) => {
            setLocalProvider(value);
            p.onProvider?.(value);
          }}
          data={providers.map((value) => ({
            value,
            label: (
              <span className="setup-provider">
                <ProviderMark provider={value} />
                {providerLabel(value)}
              </span>
            ),
          }))}
        />
      )}
      <div className="setup-accounts">
        {p.accounts
          .filter((account) => account.provider === provider)
          .map((account) => (
            <div
              key={account.id}
              className={excluded ? "setup-account-choice" : undefined}
            >
              {excluded && onExcludedChange && p.exclusionRole && (
                <Tooltip
                  label={
                    p.exclusionRole === "worker"
                      ? "Use for workers"
                      : "Use for reviewers"
                  }
                  events={{ hover: true, focus: true, touch: false }}
                >
                  <Checkbox
                    className="setup-account-exclusion"
                    aria-label={`Use ${setupAccountName(account)} for ${p.exclusionRole === "worker" ? "workers" : "reviewers"}`}
                    checked={!excluded.includes(account.id)}
                    disabled={
                      p.disabled ||
                      (p.accounts.filter((item) => !excluded.includes(item.id))
                        .length === 1 &&
                        !excluded.includes(account.id))
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
                    p.accounts.filter((item) => item.provider === provider)
                      .length === 1)
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
    </div>
  );
}

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
  if (readOnly)
    return (
      <Tooltip label={accountTooltip(account)}>
        <div
          className="setup-account setup-account-status"
          data-account-key={account.id}
          data-selected={selected || undefined}
        >
          <AccountTileLabel
            name={setupAccountName(account)}
            remaining={remaining}
          />
          {remaining !== null && (
            <AccountRemainingBar remaining={remaining} color={color} />
          )}
        </div>
      </Tooltip>
    );
  return (
    <Tooltip label={accountTooltip(account)}>
      <ActionButton
        className="setup-account"
        data-account-key={account.id}
        data-selected={selected || undefined}
        data-exhausted={remaining === 0 || undefined}
        aria-label={`Select account ${setupAccountName(account)}`}
        aria-description={
          remaining === null ? undefined : `${remaining}% remaining`
        }
        aria-pressed={selected}
        disabled={disabled}
        onClick={onSelect}
      >
        <AccountTileLabel
          name={setupAccountName(account)}
          remaining={remaining}
        >
          {multiple && selected && <Check size={13} aria-hidden="true" />}
        </AccountTileLabel>
        {remaining !== null && (
          <AccountRemainingBar remaining={remaining} color={color} />
        )}
      </ActionButton>
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

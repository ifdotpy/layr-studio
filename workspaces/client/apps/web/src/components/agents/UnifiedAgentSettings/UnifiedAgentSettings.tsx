import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";

import type { Agent } from "../../../types.ts";
import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import "../../../api.ts";
import Accounts from "../../accounts/Accounts/Accounts.tsx";
import type { AccountsState } from "../../accounts/support/Accounts.tsx";
import { ExecutionSettings } from "../ExecutionSettings/ExecutionSettings.tsx";
import { ApiError, ApiSchemaMismatchError, errorText } from "../../../api.ts";
import { serverMutationResponseIsUncertain } from "../../../servers/mutationIdentity.ts";
import type { ResourceVersion } from "../../../sync/subscriptions/contracts.ts";
import { useCallback, useEffect, useState, type ComponentProps } from "react";

export function UnifiedAgentSettingsView({
  state,
  scope,
  notify,
  onAccountModalOpenChange,
  showAccountExclusions,
  showTransferStatus = true,
  ...settings
}: Omit<
  ComponentProps<typeof ExecutionSettings>,
  "onAccountChange" | "accountDisabled" | "accounts" | "input" | "leadInput"
> & {
  agent: Agent;
  lead?: Agent | undefined;
  state: AccountsState | undefined;
  scope?: string | undefined;
  notify: (message: string) => void;
  onAccountModalOpenChange?: (opened: boolean) => void;
  showAccountExclusions?: boolean;
  showTransferStatus?: boolean;
}) {
  const store = useServerStore();
  const detailSlot = store.agentDetail(observableAgentId(settings.agent.id));
  const detail = resourceData(detailSlot.current);
  const appliedVersion = detailSlot.appliedVersion;
  const leadDetail = settings.lead
    ? resourceData(
        store.agentDetail(observableAgentId(settings.lead.id)).current,
      )
    : undefined;
  const { post } = useServerApi();

  const [accountModalOpen, setAccountModalOpen] = useState(false);
  const [exclusionsSaving, setExclusionsSaving] = useState<
    Readonly<ResourceVersion> | null | undefined
  >(null);
  useEffect(() => {
    if (exclusionsSaving === null || !appliedVersion) return;
    if (
      !exclusionsSaving ||
      appliedVersion.epoch !== exclusionsSaving.epoch ||
      appliedVersion.revision > exclusionsSaving.revision
    ) {
      setExclusionsSaving(null);
    }
  }, [appliedVersion, exclusionsSaving]);
  const showExclusions = showAccountExclusions ?? settings.agent.isLead;
  const updateAccountModalOpen = useCallback(
    (opened: boolean) => {
      setAccountModalOpen(opened);
      onAccountModalOpenChange?.(opened);
    },
    [onAccountModalOpenChange],
  );
  const saveExclusions = (
    field: "worker_excluded_account_keys" | "reviewer_excluded_account_keys",
    keys: string[],
  ) => {
    setExclusionsSaving(appliedVersion);
    void post("/api/conversation", {
      id: settings.agent.id,
      [field]: keys,
    }).catch((error) => {
      notify(errorText(error));
      if (
        error instanceof ApiError &&
        (!(error instanceof ApiSchemaMismatchError) || error.markedResponse) &&
        !serverMutationResponseIsUncertain(error.status, error.response)
      ) {
        setExclusionsSaving((pending) =>
          pending === appliedVersion ? null : pending,
        );
      }
    });
  };
  if (!state) return <p role="status">Loading accounts…</p>;
  return (
    <Accounts
      state={state}
      scope={scope}
      showTransferStatus={showTransferStatus}
      onModalOpenChange={updateAccountModalOpen}
      agent={settings.agent}
      accountKey={settings.agent.accountKey || "default"}
      onError={notify}
      changeAccount={async (key) => {
        await post("/api/agents/account", {
          id: settings.agent.id,
          account_key: key,
        });
      }}
      renderPicker={(selectAccount, disabled) => (
        <>
          <ExecutionSettings
            {...settings}
            input={{ kind: "agent", summary: settings.agent, detail }}
            leadInput={
              settings.lead
                ? { kind: "agent", summary: settings.lead, detail: leadDetail }
                : undefined
            }
            accounts={state.accounts}
            onAccountChange={selectAccount}
            accountDisabled={disabled || exclusionsSaving !== null}
            accountModalOpen={accountModalOpen}
            onWorkerExcludedAccountKeysChange={
              showExclusions
                ? (keys) => saveExclusions("worker_excluded_account_keys", keys)
                : undefined
            }
            onReviewerExcludedAccountKeysChange={
              showExclusions
                ? (keys) =>
                    saveExclusions("reviewer_excluded_account_keys", keys)
                : undefined
            }
          />
        </>
      )}
    />
  );
}
export const UnifiedAgentSettings = observer(UnifiedAgentSettingsView);

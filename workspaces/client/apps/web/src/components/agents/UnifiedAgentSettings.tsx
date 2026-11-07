import { useServerApi } from "../../servers/ServerServicesContext";
import "../../api";
import Accounts, { type useAccounts } from "../Accounts";
import { ExecutionSettings } from "./ExecutionSettings";
import { errorText } from "../../api";
import { useCallback, useState, type ComponentProps } from "react";

export function UnifiedAgentSettings({
  state,
  notify,
  onAccountModalOpenChange,
  showAccountExclusions,
  showTransferStatus = true,
  ...settings
}: Omit<
  ComponentProps<typeof ExecutionSettings>,
  "onAccountChange" | "accountDisabled" | "accounts"
> & {
  state: ReturnType<typeof useAccounts>;
  notify: (message: string) => void;
  onAccountModalOpenChange?: (opened: boolean) => void;
  showAccountExclusions?: boolean;
  showTransferStatus?: boolean;
}) {
  const { post } = useServerApi();

  const [accountModalOpen, setAccountModalOpen] = useState(false);
  const [exclusionsSaving, setExclusionsSaving] = useState(false);
  const showExclusions = showAccountExclusions ?? settings.agent.isLead;
  const updateAccountModalOpen = useCallback(
    (opened: boolean) => {
      setAccountModalOpen(opened);
      onAccountModalOpenChange?.(opened);
    },
    [onAccountModalOpenChange],
  );
  return (
    <Accounts
      state={state}
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
            accounts={state.data.accounts}
            onAccountChange={selectAccount}
            accountDisabled={disabled || exclusionsSaving}
            accountModalOpen={accountModalOpen}
            workerExcludedAccountKeys={
              showExclusions
                ? settings.agent.workerExcludedAccountKeys || []
                : undefined
            }
            reviewerExcludedAccountKeys={
              showExclusions
                ? settings.agent.reviewerExcludedAccountKeys || []
                : undefined
            }
            onWorkerExcludedAccountKeysChange={
              showExclusions
                ? (keys) => {
                    setExclusionsSaving(true);
                    void post("/api/conversation", {
                      id: settings.agent.id,
                      worker_excluded_account_keys: keys,
                    })
                      .then(() => settings.refresh())
                      .catch((error) => notify(errorText(error)))
                      .finally(() => setExclusionsSaving(false));
                  }
                : undefined
            }
            onReviewerExcludedAccountKeysChange={
              showExclusions
                ? (keys) => {
                    setExclusionsSaving(true);
                    void post("/api/conversation", {
                      id: settings.agent.id,
                      reviewer_excluded_account_keys: keys,
                    })
                      .then(() => settings.refresh())
                      .catch((error) => notify(errorText(error)))
                      .finally(() => setExclusionsSaving(false));
                  }
                : undefined
            }
          />
        </>
      )}
    />
  );
}

import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";

import type { Agent } from "../../../types.ts";
import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import "../../../api.ts";
import Accounts, {
  type AccountsController,
} from "../../accounts/Accounts/Accounts.tsx";
import { ExecutionSettings } from "../ExecutionSettings/ExecutionSettings.tsx";
import { errorText } from "../../../api.ts";
import { useCallback, useState, type ComponentProps } from "react";

export function UnifiedAgentSettingsView({
  state,
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
  state: AccountsController;
  notify: (message: string) => void;
  onAccountModalOpenChange?: (opened: boolean) => void;
  showAccountExclusions?: boolean;
  showTransferStatus?: boolean;
}) {
  const store = useServerStore();
  const detail = resourceData(
    store.agentDetail(observableAgentId(settings.agent.id)).current,
  );
  const leadDetail = settings.lead
    ? resourceData(
        store.agentDetail(observableAgentId(settings.lead.id)).current,
      )
    : undefined;
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
  const saveExclusions = (
    field: "worker_excluded_account_keys" | "reviewer_excluded_account_keys",
    keys: string[],
  ) => {
    setExclusionsSaving(true);
    void post("/api/conversation", { id: settings.agent.id, [field]: keys })
      .then(() => settings.refresh())
      .catch((error) => notify(errorText(error)))
      .finally(() => setExclusionsSaving(false));
  };
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
            input={{ kind: "agent", summary: settings.agent, detail }}
            leadInput={
              settings.lead
                ? { kind: "agent", summary: settings.lead, detail: leadDetail }
                : undefined
            }
            accounts={state.data.accounts}
            onAccountChange={selectAccount}
            accountDisabled={disabled || exclusionsSaving}
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

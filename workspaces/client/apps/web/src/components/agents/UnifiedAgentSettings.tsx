import { useServerStore } from "../../servers/ServerServicesContext.tsx";
import { resourceData } from "../../store/runtime.ts";
import { agentId as observableAgentId } from "../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";

import type { Agent } from "../../types";
import { useServerApi } from "../../servers/ServerServicesContext";
import "../../api";
import Accounts, { type useAccounts } from "../Accounts";
import { ExecutionSettings } from "./ExecutionSettings";
import { errorText } from "../../api";
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
  state: ReturnType<typeof useAccounts>;
  notify: (message: string) => void;
  onAccountModalOpenChange?: (opened: boolean) => void;
  showAccountExclusions?: boolean;
  showTransferStatus?: boolean;
}) {
  const store = useServerStore();
  const detail = settings.agent
    ? resourceData(
        store.agentDetail(observableAgentId(settings.agent!.id)).current,
      )
    : undefined;
  const leadDetail = settings.lead
    ? resourceData(
        store.agentDetail(observableAgentId(settings.lead!.id)).current,
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
export const UnifiedAgentSettings = observer(UnifiedAgentSettingsView);

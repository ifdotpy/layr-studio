import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";

import type { Agent } from "../../../types.ts";
import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import "../../../api.ts";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { AccountTransferChip } from "../../accounts/AccountTransferChip/AccountTransferChip.tsx";
import type { AccountsState } from "../../accounts/support/Accounts.tsx";
import { ExecutionSettings } from "../ExecutionSettings/ExecutionSettings.tsx";
import { ApiError, ApiSchemaMismatchError, errorText } from "../../../api.ts";
import { serverMutationResponseIsUncertain } from "../../../servers/mutationIdentity.ts";
import { type ComponentProps } from "react";
import { flow } from "mobx";

export function UnifiedAgentSettingsView({
  state,
  scope: _scope,
  notify,
  showAccountExclusions,
  showTransferStatus = true,
  ...settings
}: Omit<
  ComponentProps<typeof ExecutionSettings>,
  | "onAccountChange"
  | "accountDisabled"
  | "accounts"
  | "input"
  | "leadInput"
  | "agentsUi"
> & {
  agent: Agent;
  lead?: Agent | undefined;
  state: AccountsState | undefined;
  scope?: string | undefined;
  notify: (message: string) => void;
  showAccountExclusions?: boolean;
  showTransferStatus?: boolean;
}) {
  const store = useServerStore();
  const { accountsUi, agentsUi } = useServerAreas();
  const detailSlot = store.agentDetail(observableAgentId(settings.agent.id));
  const detail = resourceData(detailSlot.current);
  const appliedVersion = detailSlot.appliedVersion;
  const leadDetail = settings.lead
    ? resourceData(
        store.agentDetail(observableAgentId(settings.lead.id)).current,
      )
    : undefined;
  const { post } = useServerApi();

  const accountModalOpen = accountsUi.dialogOpen;
  const exclusionScope = `exclusions:${settings.agent.id}`;
  const [pendingVersion, setExclusionsSaving] = agentsUi.cell(
    exclusionScope,
    "exclusionsSaving",
  );
  const exclusionsSaving = !!(
    pendingVersion !== null &&
    (!appliedVersion ||
      (pendingVersion &&
        appliedVersion.epoch === pendingVersion.epoch &&
        appliedVersion.revision <= pendingVersion.revision))
  );
  const showExclusions = showAccountExclusions ?? settings.agent.isLead;
  const saveExclusions = flow(function* (
    field: "worker_excluded_account_keys" | "reviewer_excluded_account_keys",
    keys: string[],
  ): Generator<ReturnType<typeof post<"/api/conversation">>, void, unknown> {
    setExclusionsSaving(appliedVersion);
    try {
      yield post("/api/conversation", {
        id: settings.agent.id,
        [field]: keys,
      });
    } catch (error) {
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
    }
  });
  if (!state) return <p role="status">Loading accounts…</p>;
  return (
    <>
      <ExecutionSettings
        {...settings}
        agentsUi={agentsUi}
        input={{ kind: "agent", summary: settings.agent, detail }}
        leadInput={
          settings.lead
            ? { kind: "agent", summary: settings.lead, detail: leadDetail }
            : undefined
        }
        accounts={state.accounts}
        onAccountChange={(key) =>
          accountsUi.selectAccount(
            key,
            settings.agent,
            settings.agent.accountKey || "default",
            async (accountKey) => {
              await post("/api/agents/account", {
                id: settings.agent.id,
                account_key: accountKey,
              });
            },
          )
        }
        accountDisabled={
          !accountsUi.canSelectAccount(settings.agent) || exclusionsSaving
        }
        accountModalOpen={accountModalOpen}
        onWorkerExcludedAccountKeysChange={
          showExclusions
            ? (keys) => saveExclusions("worker_excluded_account_keys", keys)
            : undefined
        }
        onReviewerExcludedAccountKeysChange={
          showExclusions
            ? (keys) => saveExclusions("reviewer_excluded_account_keys", keys)
            : undefined
        }
      />
      {showTransferStatus && (
        <AccountTransferChip
          transfer={
            settings.agent.isLead
              ? (settings.agent.accountTransferProgress ?? undefined)
              : undefined
          }
        />
      )}
    </>
  );
}
export const UnifiedAgentSettings = observer(UnifiedAgentSettingsView);

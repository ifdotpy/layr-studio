import type { DeepReadonly } from "../../../deepReadonly.ts";
import { CircleAlert } from "lucide-react";
import {
  formatAgentStatus,
  type ChatIndicator,
} from "../../../status/operationalStatus.ts";

import { nativeErrorView } from "../../../nativeErrors.ts";

import { modelDisplayName } from "../../settings/models/support/modelOption.ts";
import type { Account } from "../../accounts/support/Accounts.tsx";
import { activityPresentation } from "../TeamRoleIcon/TeamRoleIcon.tsx";
import { accountDisplayName } from "../../../accountName.ts";
import {
  agentErrorLabel,
  agentStopReason,
  type Agent,
} from "../../../types.ts";

import type { SidebarAgent } from "../../sidebar/support/viewModel.ts";
import type { WorkerModelInfo } from "./WorkerModelPicker.tsx";

export function workerMeta(
  agent: SidebarAgent,
  detail: Agent | undefined,
  catalog: readonly WorkerModelInfo[],
  accounts: DeepReadonly<Account[]>,
) {
  const provider = agent.provider || "";
  const models = catalog.filter(
    (row) => !row.provider || row.provider === provider,
  );
  const info = models.find(
    (row) => row.model === agent.model || row.resolvedModel === agent.model,
  );
  const resolved =
    info?.isDefault && info.resolvedModel
      ? models.find(
          (row) =>
            row.model !== info.model &&
            (row.model === info.resolvedModel ||
              row.resolvedModel === info.resolvedModel),
        )
      : undefined;
  const modelName =
    !info && agent.model === "default"
      ? "Auto"
      : modelDisplayName(
          resolved ||
            (info?.isDefault && info.resolvedModel
              ? { ...info, model: info.resolvedModel }
              : info),
          agent.model || "",
        );
  const connected = accounts.filter(
    (account) => !account.disconnected && account.provider === provider,
  );
  const account = connected.find(
    (account) => account.id === (agent.accountKey || "default"),
  );
  const effort = detail?.effort || info?.defaultReasoningEffort || "default";
  const meta = [
    modelName,
    effort.charAt(0).toUpperCase() + effort.slice(1),
    connected.length > 1 && account ? accountDisplayName(account) : "",
  ]
    .filter(Boolean)
    .join(", ");
  return { provider, modelName, connected, account, effort, meta };
}

export function workerError(agent: SidebarAgent, detail: Agent | undefined) {
  return {
    errorView: nativeErrorView(detail?.error),
    stopReason: agentStopReason(agent, detail?.error),
  };
}

export function workerActivity(
  agent: SidebarAgent,
  detail: Agent | undefined,
  indicator: ChatIndicator = formatAgentStatus(agent),
) {
  const { errorView, stopReason } = workerError(agent, detail);
  const diskHeld = agent.operationalStatus.reason === "disk_space";
  const error = detail?.error ? agentErrorLabel(agent, detail?.error) : "";
  const errorSummary =
    /worktree[\s\S]*add[\s\S]*(?:exit status|exit code|failed)/i.test(error)
      ? "Could not prepare the project folder."
      : error.length > 160 ||
          /[\r\n]/.test(error) ||
          /^Command [["']/.test(error)
        ? "The agent stopped with an error."
        : error;
  const statusDetail = agent.stall
    ? "Waiting for lead: task not submitted"
    : indicator.label;
  const activity = activityPresentation(agent, detail);
  const ActivityIcon =
    error && !diskHeld && !stopReason ? CircleAlert : activity.Icon;
  const activityText =
    error && !diskHeld
      ? stopReason || errorSummary === "Could not prepare the project folder."
        ? stopReason || errorSummary
        : error.replace(/\s+/g, " ")
      : detail?.commandProgress?.[0]?.stage || statusDetail;
  return {
    indicator,
    errorView,
    diskHeld,
    error,
    stopReason,
    statusDetail,
    ActivityIcon,
    activityText,
    activity,
  };
}

import { AgentRole as GeneratedApiAgentRole } from "../../generated/api.ts";
import type { Agent } from "../../types";
import { observer } from "mobx-react-lite";
import type { SidebarAgent } from "../sidebar/viewModel.ts";
import { useServerStore } from "../../servers/ServerServicesContext.tsx";
import { resourceData } from "../../store/runtime.ts";
import { agentId } from "../../generated/observableResources.ts";

export type WorkspaceBadgeInfo = {
  label: "Layr" | "Unavailable" | "WT" | "SHARED";
  title: string;
};

export function workspaceBadgeInfo(
  agent?: Agent,
  detail = agent,
): WorkspaceBadgeInfo | null {
  if (!agent) return null;
  if (agent.legacyVmWorkspace) {
    const title = "Legacy workspace unavailable";
    return {
      label: "Unavailable",
      title: !agent.isLead && agent.cwd ? `${title} · ${agent.cwd}` : title,
    };
  }
  if (agent.remoteWorker && (!agent.cwd || !detail?.workspaceMode)) return null;
  // Legacy rows without a mode keep only the inference their flags prove.
  const mode =
    (detail?.workspaceMode as string | null | undefined) ??
    (agent.isLead
      ? undefined
      : detail?.worktree === true
        ? "worktree"
        : agent.role === GeneratedApiAgentRole.reviewer ||
            detail?.worktree === false
          ? "shared"
          : undefined);
  // A chat lead never shows the shared mode of its own folder.
  if (
    !mode ||
    !["layr-native", "layr", "worktree", "shared"].includes(mode) ||
    (agent.isLead && mode === "shared")
  )
    return null;
  const info: WorkspaceBadgeInfo =
    mode === "layr-native"
      ? { label: "Layr", title: "Native Layr workspace" }
      : mode === "layr"
        ? { label: "Unavailable", title: "Legacy workspace unavailable" }
        : mode === "worktree"
          ? { label: "WT", title: "Git worktree" }
          : { label: "SHARED", title: "Shared folder" };
  if (!agent.isLead && agent.cwd) info.title += ` · ${agent.cwd}`;
  return info;
}

export default function WorkspaceBadge({
  agent,
  passive = false,
}: {
  agent?: Agent | undefined;
  passive?: boolean | undefined;
}) {
  const info = workspaceBadgeInfo(agent);
  if (!info) return null;
  return (
    <span
      className="workspace-mode-badge"
      title={info.title}
      aria-label={info.title}
      tabIndex={passive ? undefined : 0}
    >
      {info.label}
    </span>
  );
}

export const AgentWorkspaceBadge = observer(function AgentWorkspaceBadge({
  agent,
  passive,
}: {
  agent: SidebarAgent;
  passive: boolean;
}) {
  const detail = resourceData(
    useServerStore().agentDetail(agentId(agent.id)).current,
  );
  const info = workspaceBadgeInfo(agent, detail);
  return info ? (
    <span
      className="workspace-mode-badge"
      title={info.title}
      aria-label={info.title}
      tabIndex={passive ? undefined : 0}
    >
      {info.label}
    </span>
  ) : null;
});

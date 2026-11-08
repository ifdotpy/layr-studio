import { AgentRole as GeneratedApiAgentRole } from "../../../../generated/api.ts";
import type { Agent } from "../../../../types.ts";

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

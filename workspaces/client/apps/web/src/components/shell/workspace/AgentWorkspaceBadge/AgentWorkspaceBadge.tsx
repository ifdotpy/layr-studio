import { workspaceBadgeInfo } from "../support/WorkspaceBadge.tsx";
import { observer } from "mobx-react-lite";
import type { SidebarAgent } from "../../../sidebar/support/viewModel.ts";
import { useServerStore } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../../store/runtime.ts";
import { agentId } from "../../../../generated/observableResources.ts";

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

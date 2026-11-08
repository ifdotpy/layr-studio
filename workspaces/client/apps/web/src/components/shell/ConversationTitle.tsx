import { resourceData } from "../../store/runtime.ts";
import { observer } from "mobx-react-lite";
import { ActionButton } from "../ui/primitives";
import { ArrowLeft } from "lucide-react";
import { AgentWorkspaceBadge } from "./WorkspaceBadge";
import { ChatServerLine } from "../chat-row/index.ts";
import { useServerStore } from "../../servers/ServerServicesContext.tsx";
import { useConversationActions } from "../conversation/navigation.ts";
import type { SidebarAgent } from "../sidebar/viewModel.ts";
import "../conversation/conversation-chrome.css";

export type ConversationTitleProps = {
  agent?: SidebarAgent | undefined;
  title?: string | undefined;
  passive?: boolean | undefined;
  showProject?: boolean | undefined;
  serverAlias?: string | undefined;
  serverOffline?: boolean;
};

const TitleText = observer(function TitleText({
  agent,
  title,
}: {
  agent?: SidebarAgent | undefined;
  title?: string | undefined;
}) {
  const text = agent?.name || title || "New conversation";
  return (
    <h1 id="conversation-title" title={text}>
      <span className="conversation-title-text">{text}</span>
    </h1>
  );
});
const BackToLead = observer(function BackToLead({
  agent,
}: {
  agent: SidebarAgent;
}) {
  const navigation = useConversationActions();
  return !agent.isLead && agent.rootId && agent.rootId !== agent.id ? (
    <ActionButton
      actionRole="quiet"
      size="compact-xs"
      leftSection={<ArrowLeft size={13} />}
      id="back-lead"
      aria-label="Back to main agent"
      title="Back to main agent"
      onClick={() => agent.rootId && navigation.open(agent.rootId)}
    >
      <span className="conversation-back-label">Back to main agent</span>
    </ActionButton>
  ) : null;
});
const ProjectName = observer(function ProjectName({
  agent,
}: {
  agent: SidebarAgent;
}) {
  const current = useServerStore().state().current;
  const projects = resourceData(current)?.projects;
  const name =
    [...(projects?.values() || [])].find(
      (project) => project.path === agent.projectPath,
    )?.name || agent.projectPath?.split("/").filter(Boolean).at(-1);
  return name ? <span className="conversation-project">{name}</span> : null;
});
const ServerLine = observer(function ServerLine({
  agent,
  alias,
  offline,
}: {
  agent: SidebarAgent;
  alias: string;
  offline: boolean;
}) {
  return (
    <span
      className="conversation-server"
      data-offline={offline}
      aria-label={offline ? `${alias} offline` : undefined}
    >
      <ChatServerLine
        provider={agent.provider ?? undefined}
        alias={alias}
        offline={offline}
      />
    </span>
  );
});

export default observer(function ConversationTitle({
  agent,
  title,
  passive = false,
  showProject = false,
  serverAlias,
  serverOffline = false,
}: ConversationTitleProps) {
  return (
    <div className="conversation-heading">
      {!passive && agent && <BackToLead agent={agent} />}
      <TitleText agent={agent} title={title} />
      <div className="conversation-meta">
        {serverAlias && agent && (
          <ServerLine
            agent={agent}
            alias={serverAlias}
            offline={serverOffline}
          />
        )}
        {agent && <AgentWorkspaceBadge agent={agent} passive={passive} />}
        {showProject && agent && <ProjectName agent={agent} />}
      </div>
    </div>
  );
});

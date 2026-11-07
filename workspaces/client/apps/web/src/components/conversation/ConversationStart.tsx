import "./conversation-chrome.css";
import { observer } from "mobx-react-lite";
import { useAgentSummary, useConversationStartView } from "../agents/state.ts";
import { useConversationActions } from "./navigation.ts";
import type { SidebarAgent } from "../sidebar/viewModel.ts";
import { formatAgentStatus } from "../../status/operationalStatus.ts";

const RecentChat = observer(function RecentChat({
  agent,
}: {
  agent: SidebarAgent;
}) {
  const navigation = useConversationActions();
  return (
    <button
      type="button"
      className="conversation-recent-strip"
      data-studio-state={
        formatAgentStatus(
          agent,
          navigation.read.pendingReadTargets.get(agent.id),
        ).kind
      }
      onClick={() => navigation.open(agent.id)}
      title={agent.name || "Conversation"}
    >
      <RecentTitle agent={agent} />
      <RecentStatus agent={agent} />
    </button>
  );
});
const RecentTitle = observer(function RecentTitle({
  agent,
}: {
  agent: SidebarAgent;
}) {
  return (
    <span className="conversation-recent-title">
      {agent.name || "Conversation"}
    </span>
  );
});
const RecentStatus = observer(function RecentStatus({
  agent,
}: {
  agent: SidebarAgent;
}) {
  const actions = useConversationActions();
  return (
    <span className="conversation-recent-status">
      {
        formatAgentStatus(agent, actions.read.pendingReadTargets.get(agent.id))
          .label
      }
    </span>
  );
});
const AgentConversationStart = observer(function AgentConversationStart({
  agent,
}: {
  agent: SidebarAgent;
}) {
  const view = useConversationStartView(agent);
  return (
    <section className="conversation-start" aria-label="Start a conversation">
      <header className="conversation-start-heading">
        <h2>{view.name}</h2>
        <p>Describe the work to start a conversation.</p>
      </header>
      <dl className="conversation-start-facts">
        <dt>Runs in</dt>
        <dd>{agent.projectPath || "Project location unavailable"}</dd>
      </dl>
      {!!view.recent.length && (
        <nav className="conversation-recent" aria-label="Recent project chats">
          <h3>Recent in this project</h3>
          {view.recent.map((chat) => (
            <RecentChat key={chat.id} agent={chat} />
          ))}
        </nav>
      )}
    </section>
  );
});
export const LiveConversationStart = observer(function LiveConversationStart({
  agentId,
}: {
  agentId: string;
}) {
  const agent = useAgentSummary(agentId);
  return agent ? <AgentConversationStart agent={agent} /> : null;
});

export default LiveConversationStart;

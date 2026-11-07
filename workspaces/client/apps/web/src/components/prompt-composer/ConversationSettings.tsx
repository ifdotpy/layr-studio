import { observer } from "mobx-react-lite";
import { useConversationActions } from "../conversation/navigation.ts";
import { useAgentCatalog } from "../agents/state.ts";
import { useWorkerModels } from "../agents/WorkerModelPicker.tsx";
import { UnifiedAgentSettings } from "../agents/UnifiedAgentSettings.tsx";
import { ExecutionSummaryTrigger } from "../agents/ExecutionSummaryTrigger.tsx";
import type { Agent } from "../../types.ts";

/** Account and catalog updates belong to the settings control, not the input. */
export default observer(function ConversationSettings({
  agent,
  lead,
  openRequest,
}: {
  agent: Agent;
  lead?: Agent | undefined;
  openRequest: number;
}) {
  const actions = useConversationActions();
  const { agents } = useAgentCatalog();
  const catalog = useWorkerModels(agent.accountKey || "default", true);
  return (
    <div
      className="empty-chat-settings composer-context-row"
      aria-label="Agent settings"
    >
      <UnifiedAgentSettings
        key={agent.id}
        showTransferStatus={false}
        state={actions.accounts}
        notify={actions.notify}
        refresh={actions.refresh}
        agent={agent}
        lead={lead}
        team={agents}
        catalog={catalog}
        openRequest={openRequest}
        renderTrigger={({
          agent,
          detail,
          workerDetail,
          reviewDetail,
          opened,
          ref,
          onClick,
        }) => (
          <ExecutionSummaryTrigger
            ref={ref}
            agent={agent}
            detail={detail}
            workerDetail={workerDetail}
            reviewDetail={reviewDetail}
            lead={lead}
            team={agents}
            aria-expanded={opened}
            onOpen={onClick}
          />
        )}
      />
    </div>
  );
});

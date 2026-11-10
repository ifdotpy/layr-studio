import { ConversationHeaderTools } from "../ConversationHeaderTools/ConversationHeaderTools.tsx";
import { ConversationTranscript } from "../ConversationTranscript/ConversationTranscript.tsx";
import { ConversationFollow } from "../ConversationFollow/ConversationFollow.tsx";
import { ConversationQueueArea } from "../ConversationQueueArea/ConversationQueueArea.tsx";
import { ConversationComposerArea } from "../ConversationComposerArea/ConversationComposerArea.tsx";
import { ConversationFooter } from "../ConversationFooter/ConversationFooter.tsx";
import { ConversationOperations } from "../support/ConversationStore.ts";
import { useAttachmentDraftStore } from "../support/useAttachmentDrafts.ts";
import { useConversationActions } from "../support/navigation.ts";
import { AgentSource } from "../../../generated/api.ts";
import { useSendingMessageRemoval } from "../support/useSendingMessageRemoval.ts";
import {
  useServerApi,
  useServerOutbox,
  useServerUploads,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useServerActivity } from "../../../servers/activity.ts";
import { useMessageQueue } from "../support/useMessageQueue.ts";
import { useEffect, useRef, useState } from "react";
import { useConversationScroll } from "../support/useConversationScroll.ts";
import { useUploadRecovery } from "../support/useUploadRecovery.ts";
import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
export function useConversationContext() {
  const actions = useConversationActions();
  const { selected, navigation, startup } = actions;
  const opened = navigation.selectedId,
    kind = selected.kind,
    stateDir = startup.creationScope || "";
  const input = useRef<HTMLTextAreaElement>(null);
  const scrolling = useConversationScroll(
    `${stateDir}:${kind}:${opened}`,
    selected.history.loaded,
    !selected.history.after,
    `studio-turns:${stateDir}:${opened}`,
  );
  const attachmentDrafts = useAttachmentDraftStore(
    `codex-agent-attachments:${stateDir}`,
    actions.notices.notify,
  );
  const uploadRecovery = useUploadRecovery(
    stateDir,
    startup.workspaceId || "",
    attachmentDrafts.set,
  );
  const api = useServerApi(),
    { changeOutbox } = useServerOutbox(),
    { queueUploads } = useServerUploads();
  const { removeSendingMessage } = useSendingMessageRemoval();
  const messageQueue = useMessageQueue({
    id: opened,
    enabled: navigation.selectedAgent?.source === AgentSource.managed,
    scope: `${stateDir}:${startup.workspaceId || ""}:${opened}`,
    workspaceId: startup.workspaceId,
    observed: actions.delivery.observeSends,
    edited: actions.delivery.editSend,
  });
  const [operations] = useState(() => new ConversationOperations());
  return {
    actions,
    opened,
    stateDir,
    kind,
    api,
    changeOutbox,
    queueUploads,
    removeSendingMessage,
    messageQueue,
    uploadRecovery,
    attachmentDrafts,
    input,
    operations,
    scrolling,
  };
}
export function ConversationView() {
  const context = useConversationContext();
  const {
    actions: { selected, navigation },
    uploadRecovery,
  } = context;
  const control = selected.openConversation(context);
  useEffect(control.start, [control]);
  useServerActivity(control.addingFiles || uploadRecovery.pending.length > 0);
  reportPromptComposerRender("conversation");
  return (
    <section
      id="conversation"
      className={
        navigation.selectedRoom ? "agent-conversation" : "ai-conversation"
      }
    >
      <ConversationHeaderTools />
      <ConversationTranscript />
      <ConversationFollow />
      <ConversationQueueArea />
      <ConversationComposerArea />
      <ConversationFooter />
    </section>
  );
}

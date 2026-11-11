import { ConversationHeaderTools } from "../ConversationHeaderTools/ConversationHeaderTools.tsx";
import { ConversationTranscript } from "../ConversationTranscript/ConversationTranscript.tsx";
import { ConversationFollow } from "../ConversationFollow/ConversationFollow.tsx";
import { ConversationQueueArea } from "../ConversationQueueArea/ConversationQueueArea.tsx";
import { ConversationComposerArea } from "../ConversationComposerArea/ConversationComposerArea.tsx";
import { ConversationFooter } from "../ConversationFooter/ConversationFooter.tsx";
import { useConversationActions } from "../support/navigation.ts";
import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
export function ConversationView() {
  const { navigation } = useConversationActions();
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
export { ConversationView as renderConversationView };

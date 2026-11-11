import { observer } from "mobx-react-lite";
import { useConversationActions } from "../support/navigation.ts";
import { ConversationPromptNavigation } from "../ConversationPromptNavigation/ConversationPromptNavigation.tsx";

export const ConversationMenuTools = observer(function ConversationMenuTools() {
  const { selected } = useConversationActions();
  return selected.conversation ? <ConversationPromptNavigation /> : null;
});

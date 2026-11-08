import { observer } from "mobx-react-lite";
import ConversationTitle from "../../components/shell/ConversationTitle.tsx";
import { useConversationActions } from "../../components/conversation/navigation.ts";
export const SelectedTitle = observer(function SelectedTitle() {
  const area = useConversationActions();
  const { selected, navigation, layout } = area;
  return (
    <ConversationTitle
      agent={navigation.selectedAgent}
      title={selected.conversationTitle}
      passive={layout.usesMenuHeaderTools}
      showProject={!!layout.railGeometry}
      serverAlias={selected.serverAlias}
      serverOffline={selected.serverOffline}
    />
  );
});

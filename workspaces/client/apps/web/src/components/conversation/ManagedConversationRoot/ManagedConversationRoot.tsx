import { observer } from "mobx-react-lite";
import Conversation from "../ObservedConversation/ObservedConversation.tsx";
import RadioChat from "../RadioChat/RadioChat.tsx";
import UIErrorBoundary from "../../shell/UIErrorBoundary/UIErrorBoundary.tsx";
import { useConversationActions } from "../support/navigation.ts";

/** Selection is resolved once; the widget receives a stable owner reference. */
export const ManagedConversationRoot = observer(
  function ManagedConversationRoot() {
    const area = useConversationActions();
    const { selected, navigation } = area;
    return (
      <UIErrorBoundary
        label="this conversation"
        resetKey={navigation.selectedId}
      >
        {selected.isSelectionSuspended ? (
          <div className="chat-empty-state" role="status">
            {selected.deletionMessage}
          </div>
        ) : navigation.selectedRoom?.radio ? (
          <RadioChat room={navigation.selectedRoom} />
        ) : (
          <Conversation key={navigation.selectedId || "new"} />
        )}
      </UIErrorBoundary>
    );
  },
);

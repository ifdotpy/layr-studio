import { observer } from "mobx-react-lite";
import { useConversationStore } from "../support/ConversationStore.ts";
import PromptNavigator from "../PromptNavigator/PromptNavigator.tsx";
import { errorText } from "../../../api.ts";
export const ConversationPromptNavigation = observer(
  function ConversationPromptNavigation() {
    const control = useConversationStore();
    const { actions, opened, stateDir, scrolling } = control.context;
    const { layout, navigation, selected, notices } = actions;
    const { before, setPageAnchor, older, pageLoading } = selected.history;
    return !navigation.selectedRoom && opened ? (
      <PromptNavigator
        navigationStore={control.promptNavigation}
        compact
        inMenu={layout.usesMenuHeaderTools}
        key={opened}
        messages={selected.visibleMessages}
        loadOlder={
          before
            ? () => {
                scrolling.setFollow(false);
                setPageAnchor(scrolling.getAnchorId());
                void older().catch((error) => notices.notify(errorText(error)));
              }
            : undefined
        }
        loadingOlder={pageLoading}
        agentId={control.managed ? opened : undefined}
        container={scrolling.scroll}
        storageKey={`studio-prompt-bookmarks:${stateDir}:${opened}`}
        jump={control.jumpToPrompt}
      />
    ) : null;
  },
);

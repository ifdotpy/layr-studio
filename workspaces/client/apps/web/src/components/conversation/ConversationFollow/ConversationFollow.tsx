import { useConversationStore } from "../support/ConversationStore.ts";
import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import { ArrowDown } from "lucide-react";
import SelectionQuote from "../transcript/SelectionQuote/SelectionQuote.tsx";
export function renderConversationFollow() {
  const control = useConversationStore();
  const { scroll } = control.context.scrolling;
  const { actions, opened } = control.context;
  return (
    <>
      {!actions.navigation.selectedRoom && (
        <SelectionQuote
          key={`selection:${opened}`}
          chatId={opened}
          root={scroll}
          onQuote={control.quote}
        />
      )}

      <FollowLatest />
    </>
  );
}

export const ConversationFollow = observer(renderConversationFollow);

const FollowLatest = observer(function FollowLatest() {
  const control = useConversationStore();
  const following = control.context.scrolling.getFollow();
  return following ? null : (
    <div className="jump-slot">
      <Button
        id="jump-latest"
        className="jump"
        variant="default"
        radius="xl"
        leftSection={<ArrowDown size={14} />}
        onClick={control.returnToLatest}
      >
        Latest
      </Button>
    </div>
  );
});

import { useConversationStore } from "../support/ConversationStore.ts";
import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import { ArrowDown } from "lucide-react";
import { useSyncExternalStore } from "react";
import SelectionQuote from "../transcript/SelectionQuote/SelectionQuote.tsx";
export function renderConversationFollow() {
  const control = useConversationStore();
  const { subscribeFollow, getFollow, scroll } = control.context.scrolling;
  const { actions, opened } = control.context;
  const navigation = actions.navigation;
  const room = navigation.selectedRoom;
  const history = control;
  const returnToLatest = history.returnToLatest;
  return (
    <>
      {!room && (
        <SelectionQuote
          key={`selection:${opened}`}
          chatId={opened}
          root={scroll}
          onQuote={control.quote}
        />
      )}

      <FollowLatest
        subscribe={subscribeFollow}
        getSnapshot={getFollow}
        onClick={returnToLatest}
      />
    </>
  );
}

export const ConversationFollow = observer(renderConversationFollow);

function FollowLatest({
  subscribe,
  getSnapshot,
  onClick,
}: {
  subscribe: (listener: () => void) => () => void;
  getSnapshot: () => boolean;
  onClick: () => void;
}) {
  const following = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return following ? null : (
    <div className="jump-slot">
      <Button
        id="jump-latest"
        className="jump"
        variant="default"
        radius="xl"
        leftSection={<ArrowDown size={14} />}
        onClick={onClick}
      >
        Latest
      </Button>
    </div>
  );
}

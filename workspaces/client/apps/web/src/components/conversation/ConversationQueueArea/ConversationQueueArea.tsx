import { useConversationStore } from "../support/ConversationStore.ts";
import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import { errorText } from "../../../api.ts";
import { isCyberPolicyRefusal } from "../../../nativeErrors.ts";
import MessageQueue from "../MessageQueue/MessageQueue.tsx";
export function renderConversationQueueArea() {
  const control = useConversationStore();
  const { actions, messageQueue } = control.context;
  const { navigation, notices, selected } = actions;
  const { selectedRoom: room, selectedAgent: agent } = navigation;
  const managed = control.managed;
  const detail = selected.detail;
  if (room) return null;
  return (
    <>
      {managed && (
        <>
          {Boolean(messageQueue.pending) && !messageQueue.busy && (
            <div className="notice queue-notice" role="status">
              The last queue change needs confirmation.
              <Button
                size="compact-xs"
                onClick={() =>
                  void messageQueue
                    .retry()
                    .catch((error) => notices.notify(errorText(error)))
                }
              >
                Retry queue change
              </Button>
            </div>
          )}
          <MessageQueue store={control.queueEditor} />
        </>
      )}
      {agent && isCyberPolicyRefusal(detail?.error) && (
        <div className="native-policy-composer-warning" role="note">
          <strong>Before you continue</strong>
          <span>
            Continuing this chat sends the same history and can be refused
            again.
          </span>
        </div>
      )}
    </>
  );
}

export const ConversationQueueArea = observer(renderConversationQueueArea);

import { useConversationStore } from "../support/ConversationStore.ts";
import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import { errorText } from "../../../api.ts";
import { isCyberPolicyRefusal } from "../../../nativeErrors.ts";
import { providerSupports } from "../../../generated/providerKind.ts";
import { mergeQueueOrder } from "../support/messageDelivery.ts";
import MessageQueue from "../MessageQueue/MessageQueue.tsx";
export function renderConversationQueueArea() {
  const control = useConversationStore();
  const { actions, messageQueue, stateDir, opened } = control.context;
  const { navigation, notices, startup, selected } = actions;
  const { selectedRoom: room, selectedAgent: agent } = navigation;
  const managed = control.managed;
  const queueScope = `${stateDir}:${startup.workspaceId || ""}:${opened}`;
  const detail = selected.detail;
  if (room) return null;
  const controls = { steer: providerSupports(agent?.provider, "steer") };
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
          <MessageQueue
            loading={messageQueue.loading}
            items={control.nextTurnQueue}
            scope={queueScope}
            onEdit={messageQueue.edit}
            onCancel={messageQueue.cancel}
            onSendNow={controls.steer ? messageQueue.sendNow : undefined}
            onReorder={(ids) =>
              messageQueue.reorder(
                mergeQueueOrder(
                  messageQueue.items.map((item) => item.id),
                  control.nextTurnQueue.map((item) => item.id),
                  ids,
                ),
              )
            }
            canReorder={
              messageQueue.canReorder &&
              !control.nextTurnQueue.some((item) => item.localDelivery)
            }
            refreshing={messageQueue.busy}
            error={messageQueue.error}
          />
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

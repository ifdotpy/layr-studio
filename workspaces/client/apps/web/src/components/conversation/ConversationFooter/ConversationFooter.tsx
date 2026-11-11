import { action } from "mobx";
import { useConversationStore } from "../support/ConversationStore.ts";
import { observer } from "mobx-react-lite";
import { AgentSource, RoomKind } from "../../../generated/api.ts";
import Usage from "../Usage/Usage.tsx";
export function renderConversationFooter() {
  const control = useConversationStore();
  const navigation = control.context.actions.navigation;
  const { selectedRoom: room, selectedAgent: agent } = navigation;
  return room ? (
    <p className="room-footer">
      {room.kind === RoomKind.private
        ? "Private agent chat."
        : "Broadcast to " +
          (room.rootId === "all"
            ? "all teams (historical, read only)."
            : "this team.")}
    </p>
  ) : (
    <div className="conversation-composer-footer">
      {agent?.source === AgentSource.managed && (
        <Usage
          agent={agent}
          usageStore={control.usageStore}
          opened={control.limitsOpen}
          onChange={action((value) => {
            control.limitsOpen = value;
          })}
          openRequest={control.limitsOpenRequest}
        />
      )}
    </div>
  );
}

export const ConversationFooter = observer(renderConversationFooter);

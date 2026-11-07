import { type ChatRowProps } from "./actions.ts";
import { observer } from "mobx-react-lite";
import { WifiOff } from "lucide-react";
import { offlineServerDescription } from "./ChatServerLine.tsx";

export const ChatRowMeta = observer(function ChatRowMeta({
  state,
}: ChatRowProps) {
  const row = state.row;
  const {
    show: showServer,
    offline: serverOffline,
    alias: serverAlias,
  } = state.server;
  const offlineDescription = serverOffline
    ? offlineServerDescription(serverAlias)
    : undefined;
  return (
    (showServer || serverOffline || row.waitingForLead) && (
      <span className="fleet-chat-meta">
        {(showServer || serverOffline) && (
          <span
            className="fleet-server-badge"
            data-offline={serverOffline || undefined}
            title={offlineDescription || serverAlias}
            aria-label={
              serverOffline ? offlineDescription : `Server ${serverAlias}`
            }
          >
            {serverOffline && <WifiOff size="1em" aria-hidden="true" />}
            {showServer ? serverAlias : "Offline"}
          </span>
        )}
        {row.waitingForLead && (
          <span className="agent-stall" role="status">
            Waiting for lead
          </span>
        )}
      </span>
    )
  );
});

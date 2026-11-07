import { observer } from "mobx-react-lite";
import { WifiOff } from "lucide-react";
import type { SidebarChatRow } from "../../generated/observableResources.ts";
import { offlineServerDescription } from "./ChatServerLine.tsx";

export const ChatRowMeta = observer(function ChatRowMeta({
  row,
  showServer,
  serverOffline,
  serverAlias,
}: {
  row: Pick<SidebarChatRow, "waitingForLead">;
  showServer: boolean;
  serverOffline: boolean;
  serverAlias: string;
}) {
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

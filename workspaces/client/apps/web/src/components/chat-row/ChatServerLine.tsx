import { observer } from "mobx-react-lite";
import { Badge, Tooltip } from "@mantine/core";
import { WifiOff } from "lucide-react";
import { ProviderMark } from "../AccountTiles.tsx";
import { providerLabel } from "../../generated/providerKind.ts";
import type { SidebarChatRow } from "../../generated/observableResources.ts";
import "../../servers/sidebar-status.css";

export const ChatServerLine = observer(function ChatServerLine({
  row,
  provider: legacyProvider,
  alias,
  offline = false,
}: {
  row?: Partial<Pick<SidebarChatRow, "provider">> | undefined;
  provider?: string | undefined;
  alias: string;
  offline?: boolean | undefined;
}) {
  const provider = row ? row.provider : legacyProvider;
  return (
    <span
      className="chat-server-line"
      aria-label={`${providerLabel(provider || "")}, ${alias}`}
    >
      <ProviderMark provider={provider || ""} />
      <span className="chat-server-alias" title={alias}>
        {alias}
      </span>
      {offline && (
        <span className="chat-server-offline-wrap">
          <Tooltip
            label={offlineServerDescription(alias)}
            position="right"
            withinPortal
            events={{ hover: true, focus: true, touch: false }}
          >
            <Badge
              className="chat-server-offline"
              size="xs"
              color="gray"
              variant="transparent"
              leftSection={<WifiOff size={11} aria-hidden="true" />}
              aria-label={offlineServerDescription(alias)}
              tabIndex={0}
            >
              <span className="chat-server-offline-label">Offline</span>
            </Badge>
          </Tooltip>
        </span>
      )}
    </span>
  );
});

export function offlineServerDescription(alias: string) {
  return `${alias} is unreachable. Showing the last known state.`;
}

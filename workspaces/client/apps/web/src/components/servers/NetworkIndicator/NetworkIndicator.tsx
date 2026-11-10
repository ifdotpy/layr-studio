import { Tooltip } from "@mantine/core";
import { observer } from "mobx-react-lite";
import type { Net } from "../../../servers/net/net.ts";

const labels = {
  live: "Connected",
  offline: "Offline",
  reconnecting: "Reconnecting",
  "schema-mismatch": "Update required",
} as const;

/** A colored dot for the server connection; its tooltip opens on hover and focus. */
export const NetworkIndicator = observer(function NetworkIndicator({
  net,
}: {
  net: Net;
}) {
  const { connection, lastError } = net.status;
  const label = `Server connection: ${labels[connection]}`;
  return (
    <Tooltip
      label={lastError ? `${label}. ${lastError}` : label}
      events={{ hover: true, focus: true, touch: true }}
    >
      <span
        className="network-dot"
        role="status"
        tabIndex={0}
        aria-label={lastError ? `${label}. ${lastError}` : label}
        data-connection={connection}
      />
    </Tooltip>
  );
});

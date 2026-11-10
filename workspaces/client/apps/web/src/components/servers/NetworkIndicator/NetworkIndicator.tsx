import { observer } from "mobx-react-lite";
import type { Net } from "../../../servers/net/net.ts";

export const NetworkIndicator = observer(function NetworkIndicator({
  net,
}: {
  net: Net;
}) {
  const { connection, inFlight, lastError } = net.status;
  return (
    <span
      role="status"
      aria-label="Server connection"
      data-connection={connection}
      title={lastError || undefined}
    >
      {connection === "schema-mismatch"
        ? "Update required"
        : connection === "live"
          ? "Live"
          : connection === "offline"
            ? "Offline"
            : "Reconnecting"}
      {inFlight > 0 && ` · ${inFlight} active requests`}
    </span>
  );
});

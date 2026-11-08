import { useServerOutbox } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button } from "@mantine/core";
import { useState } from "react";
import { errorText } from "../../../api.ts";
import type { OutgoingMessage } from "../../../sync/send.ts";
import { MessageSendStatus } from "../../../generated/api.ts";

export default function OutboxControls({
  entry,
  onResolved,
}: {
  entry?: OutgoingMessage | undefined;
  onResolved?: (id: string) => void;
}) {
  const { changeOutbox } = useServerOutbox();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  // Only a send that needs the user's decision shows actions. A send in flight
  // has its own remove control, so its bubble keeps one size until delivery.
  if (
    !entry ||
    !(
      (entry.status.source === "device" &&
        (entry.status.state.kind === "transport_failed" ||
          entry.status.state.kind === "receipt_expired")) ||
      (entry.status.source === "server" &&
        (entry.status.status === MessageSendStatus.uncertain ||
          entry.status.status === MessageSendStatus.failed))
    )
  )
    return null;
  const run = async (action: "resume" | "cancel") => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const result = await changeOutbox(entry.id, action);
      if (
        action === "cancel" ||
        (result &&
          result.kind === "server" &&
          result.status !== MessageSendStatus.uncertain &&
          result.status !== MessageSendStatus.failed)
      )
        onResolved?.(entry.id);
    } catch (failure) {
      setError(errorText(failure));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="outbox-controls">
      <div className="outbox-buttons">
        {!(
          entry.status.source === "device" &&
          entry.status.state.kind === "receipt_expired"
        ) && (
          <Button
            size="compact-xs"
            disabled={busy}
            onClick={() => void run("resume")}
          >
            Retry
          </Button>
        )}
        <Button
          size="compact-xs"
          disabled={busy}
          onClick={() => void run("cancel")}
        >
          Discard
        </Button>
      </div>
      {error && <p role="alert">{error}</p>}
    </div>
  );
}

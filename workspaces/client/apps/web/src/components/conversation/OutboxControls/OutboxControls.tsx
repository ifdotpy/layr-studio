import { useServerOutbox } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button } from "@mantine/core";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { AsyncFeedback } from "../../../stores/AsyncFeedback.ts";
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
  return (
    <Owned create={() => new AsyncFeedback()}>
      {(state) => {
        const status = entry?.status;
        const kind =
          status?.source === "device" ? status.state.kind : status?.status;
        if (
          !entry ||
          ![
            "transport_failed",
            "receipt_expired",
            "uncertain",
            "failed",
          ].includes(kind || "")
        )
          return null;
        const choices =
          kind === "receipt_expired"
            ? (["cancel"] as const)
            : (["resume", "cancel"] as const);

        const run = (action: "resume" | "cancel") =>
          state.run(
            () => changeOutbox(entry.id, action),
            (result) => {
              if (
                action === "cancel" ||
                (result?.kind === "server" &&
                  result.status !== MessageSendStatus.uncertain &&
                  result.status !== MessageSendStatus.failed)
              )
                onResolved?.(entry.id);
            },
          );
        return (
          <div className="outbox-controls">
            <div className="outbox-buttons">
              {choices.map((action) => (
                <Button
                  key={action}
                  size="compact-xs"
                  disabled={state.pending}
                  onClick={() => void run(action)}
                >
                  {action === "resume" ? "Retry" : "Discard"}
                </Button>
              ))}
            </div>
            {state.error && <p role="alert">{state.error}</p>}
          </div>
        );
      }}
    </Owned>
  );
}

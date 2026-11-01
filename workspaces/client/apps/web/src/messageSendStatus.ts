import { MessageSendStatus } from "./generated/api.ts";
import type { Message } from "./types.ts";
import type { DeliveryState } from "./sync/send.ts";

export function serverStatus(value: DeliveryState | undefined) {
  return value?.source === "server" ? value.status : undefined;
}

/** Whether a message is still awaiting delivery. */
export function isMessagePending(message: Message): boolean {
  const status = message.deliveryStatus;
  return (
    message.role === "user" &&
    (status?.source === "device"
      ? status.state.kind === "stored" || status.state.kind === "in_flight"
      : status?.status === MessageSendStatus.queued ||
        status?.status === MessageSendStatus.sending)
  );
}

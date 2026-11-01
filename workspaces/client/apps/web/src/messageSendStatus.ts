import { MessageSendStatus } from "./generated/api.ts";
import type { Message } from "./types.ts";

/** Whether a message is still awaiting delivery. */
export function isMessagePending(message: Message): boolean {
  return (
    message.role === "user" &&
    (message.deliveryStatus === MessageSendStatus.buffered ||
      message.deliveryStatus === MessageSendStatus.queued ||
      message.deliveryStatus === MessageSendStatus.sending)
  );
}

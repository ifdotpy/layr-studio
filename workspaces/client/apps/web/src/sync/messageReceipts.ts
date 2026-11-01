import { MessageSendStatus } from "../generated/api.ts";
import type { GetResult } from "../api";
import type { Message } from "../types";
import type { OutgoingMessage } from "./send";

type MessageReceiptResponse = GetResult<"/api/messages/receipts">;
export type MessageReceipt = MessageReceiptResponse["items"][number];

export function checkedMessageReceipts(
  value: MessageReceiptResponse,
  room: string,
  ids: string[],
  workspaceId?: string,
): MessageReceipt[] {
  if (workspaceId && value.workspaceId !== workspaceId)
    throw new Error("The delivery receipts belong to another workspace.");
  const requested = new Set(ids);
  if (value.agent !== room)
    throw new Error("The delivery receipts belong to another chat.");
  const receipts: MessageReceipt[] = [];
  for (const item of value.items) {
    if (!requested.has(item.id))
      throw new Error(
        "The delivery receipts do not match the requested messages.",
      );
    receipts.push(item);
  }
  if (new Set(receipts.map((item) => item.id)).size !== receipts.length)
    throw new Error(
      "The delivery receipts do not match the requested messages.",
    );
  return receipts;
}

export function latestMessageReceipt(
  previous: MessageReceipt | undefined,
  next: MessageReceipt,
) {
  return previous?.status === MessageSendStatus.delivered &&
    next.status !== MessageSendStatus.delivered
    ? previous
    : next;
}

export function receiptOutgoing(
  entry: OutgoingMessage,
  receipt?: MessageReceipt,
): OutgoingMessage;
export function receiptOutgoing(
  entry: Omit<OutgoingMessage, "id">,
  receipt?: MessageReceipt,
): Omit<OutgoingMessage, "id">;
export function receiptOutgoing(
  entry: Omit<OutgoingMessage, "id">,
  receipt?: MessageReceipt,
): Omit<OutgoingMessage, "id"> {
  if (!receipt) return entry;
  const current = latestMessageReceipt(entry.receipt, receipt);
  return {
    ...entry,
    status: current.status,
    receipt: current,
    error: current.error || undefined,
  };
}

export function receiptTranscript(
  items: Message[],
  room: string,
  receipts: Map<string, MessageReceipt>,
) {
  return items.map((item) => {
    if (item.role !== "user") return item;
    const id =
      item.clientMessageId ||
      (item.id.startsWith(room + ":")
        ? item.id.slice(room.length + 1)
        : item.id);
    const receipt = receipts.get(id);
    if (!receipt) return item;
    const status =
      item.deliveryStatus === MessageSendStatus.delivered
        ? MessageSendStatus.delivered
        : receipt.status;
    const legacy: Message & { pending?: unknown } = item;
    const { pending: legacyPending, ...message } = legacy;
    void legacyPending;
    return {
      ...message,
      deliveryStatus: status,
      deliveryError:
        status === MessageSendStatus.delivered
          ? undefined
          : receipt.error || undefined,
    };
  });
}

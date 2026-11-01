import {
  DeliveryMoment,
  normalizeDelivery,
} from "../../generated/deliveryMoment";
import type { Message } from "../../types";
import type { OutboxEntry, OutgoingMessage } from "../../sync/send";
import { MessageSendStatus } from "../../generated/api";
import { serverStatus } from "../../messageSendStatus";
import type { DeliveryState } from "../../sync/send";

const deliveryLabels: Record<MessageSendStatus, string> = {
  queued: "Queued",
  sending: "Sending…",
  delivered: "Sent",
  uncertain: "Delivery unconfirmed",
  failed: "Not sent",
  cancelled: "Cancelled",
  stored_only: "Stored",
};

const deviceLabels: Record<
  Extract<DeliveryState, { source: "device" }>["state"]["kind"],
  string
> = {
  stored: "Stored on device",
  in_flight: "Sending…",
  transport_failed: "Could not contact server",
  receipt_expired: "Delivery outcome unknown",
  removed: "Removed from device",
};

export function explicitQueue(item: Message) {
  return (
    (item.deliveryStatus?.source === "device" &&
      (item.deliveryStatus.state.kind === "stored" ||
        item.deliveryStatus.state.kind === "in_flight")) ||
    serverStatus(item.deliveryStatus) === MessageSendStatus.queued
  );
}

export function dispatchedMessage(item: Message) {
  return !explicitQueue(item);
}

// Hidden scheduler inputs keep their original slots and the full queue revision.
export function mergeQueueOrder(
  raw: string[],
  visible: string[],
  ordered: string[],
) {
  const selected = new Set(visible);
  if (
    new Set(ordered).size !== selected.size ||
    ordered.length !== visible.length ||
    ordered.some((id) => !selected.has(id)) ||
    visible.some((id) => !raw.includes(id))
  )
    throw new Error("The queue changed. Reload before reordering.");
  let index = 0;
  return raw.map((id) => (selected.has(id) ? ordered[index++]! : id));
}

export const messageRenderKey = (item: Message) =>
  item.role === "user" && item.clientMessageId
    ? `client:${item.clientMessageId}`
    : item.id;

export function receiptMessage(item: Message, entry: OutgoingMessage) {
  const key = `${entry.body.room}:${entry.id}`;
  return (
    item.clientMessageId === entry.id ||
    item.id === entry.id ||
    item.id === key ||
    (!item.clientMessageId && item.id === `${key}:0`)
  );
}

/** Merge durable outbox entries with this tab's display copies of its sends. */
export function mergeOutgoing(
  outbox: OutboxEntry[],
  local: Record<string, OutgoingMessage>,
  observed: ReadonlySet<string>,
) {
  const stored = new Map(outbox.map((entry) => [entry.id, entry]));
  const visibleSends = new Map(outbox.map((entry) => [entry.id, entry]));
  // The outbox owns delivery state once it stores a message. A local copy
  // shows only before that.
  for (const entry of Object.values(local)) {
    const owner = stored.get(entry.id);
    if (
      !owner ||
      (entry.status.source === "server" && owner.status.source === "device")
    )
      visibleSends.set(entry.id, entry);
  }
  return [...visibleSends.values()].filter((entry) => !observed.has(entry.id));
}

export function outgoingTranscript(
  items: Message[],
  entries: OutgoingMessage[],
) {
  const observed: string[] = [];
  const extra: Message[] = [];
  const updates = new Map<string, Message>();
  for (const entry of entries) {
    const present = items.find((item) => receiptMessage(item, entry));
    if (present) {
      let shown =
        !present.assets?.length &&
        entry.attachments?.length &&
        present.materialized === false
          ? { ...present, assets: entry.attachments }
          : present;
      if (!shown.requestedDelivery && entry.body.delivery)
        shown = {
          ...shown,
          requestedDelivery: normalizeDelivery(
            entry.body.delivery ?? DeliveryMoment.CurrentTurn,
          ),
        };
      updates.set(present.id, shown);
      if (!shown.deliveryStatus || entry.status.source === "server")
        updates.set(present.id, {
          ...shown,
          deliveryStatus: entry.status,
          deliveryError:
            entry.status.source === "device" &&
            (entry.status.state.kind === "transport_failed" ||
              entry.status.state.kind === "receipt_expired")
              ? entry.status.state.error
              : entry.receipt?.error,
        });
      if (
        present.deliveryStatus?.source === "server" &&
        !explicitQueue(present) &&
        present.materialized !== false
      )
        observed.push(entry.id);
      continue;
    }
    // An input outside the loaded page can replace its local card only after
    // the server confirms both delivery and the exact saved transcript input.
    if (
      serverStatus(entry.status) === MessageSendStatus.delivered &&
      entry.receipt?.status === MessageSendStatus.delivered &&
      entry.receipt.materialized === true
    ) {
      observed.push(entry.id);
      continue;
    }
    extra.push({
      id: `${entry.body.room}:${entry.id}`,
      clientMessageId: entry.id,
      role: "user",
      title: "You",
      text: entry.displayText ?? entry.body.text,
      assets: entry.attachments || [],
      at: entry.created / 1000,
      deliveryStatus: entry.status,
      deliveryError:
        entry.status.source === "device" &&
        (entry.status.state.kind === "transport_failed" ||
          entry.status.state.kind === "receipt_expired")
          ? entry.status.state.error
          : entry.receipt?.error,
      localDelivery: true,
      requestedDelivery: normalizeDelivery(
        entry.body.delivery ?? DeliveryMoment.CurrentTurn,
      ),
    });
  }
  return {
    items: [...items.map((item) => updates.get(item.id) || item), ...extra],
    observed,
  };
}

export function deliveryLabel(item: Message) {
  const delivery = item.deliveryStatus;
  return !delivery
    ? ""
    : delivery.source === "server"
      ? deliveryLabels[delivery.status]
      : deviceLabels[delivery.state.kind];
}

import { MessageSendStatus } from "../../generated/api.ts";
import { DeliveryMoment, DeliveryMomentInput } from "../../generated/api";
import { required } from "../componentTestFixtures";
import {
  componentMessage,
  componentAttachment,
} from "../componentTestFixtures";
import type { Message } from "../../types";
import type { OutgoingMessage, OutboxEntry } from "../../sync/send";
import assert from "node:assert/strict";
import {
  mergeOutgoing,
  outgoingTranscript,
  deliveryLabel,
  explicitQueue,
  dispatchedMessage,
  mergeQueueOrder,
  messageRenderKey,
  receiptMessage,
} from "./messageDelivery.ts";
import { it } from "vitest";

// The saved receipt fixture omits body.id and receipt.id.
const receipt = {
  id: "one",
  body: { id: "one", room: "lead", text: "Same text" },
  created: 1,
  status: { source: "server", status: MessageSendStatus.delivered },
  receipt: { id: "one", status: "queued" },
} satisfies OutgoingMessage;
const pending: Message = {
  id: "lead:one",
  role: "user",
  text: "Same text",
  deliveryStatus: { source: "server", status: MessageSendStatus.queued },
  materialized: false,
};

it("matches transcript rows by receipt identity and retains edited text and attachments", () => {
  assert.equal(
    required(
      required(
        outgoingTranscript(
          [pending],
          [
            {
              ...receipt,
              attachments: [componentAttachment({ id: "asset-one" })],
            },
          ],
        ).items[0],
      ).assets?.[0],
    ).id,
    "asset-one",
  );
  assert.equal(
    required(outgoingTranscript([], [receipt]).items[0]).text,
    "Same text",
  );
  assert.deepEqual(outgoingTranscript([pending], [receipt]).observed, []);
  assert.equal(outgoingTranscript([pending], [receipt]).items.length, 1);
  // An older server omits a reserved queue row before it materializes the input.
  assert.equal(outgoingTranscript([], [receipt]).items.length, 1);
  assert.deepEqual(
    outgoingTranscript([{ ...pending, materialized: false }], [receipt])
      .observed,
    [],
  );
  assert.deepEqual(
    outgoingTranscript(
      [
        {
          ...pending,
          materialized: true,
          deliveryStatus: {
            source: "server",
            status: MessageSendStatus.delivered,
          },
        },
      ],
      [receipt],
    ).observed,
    ["one"],
  );
  // Equal text does not identify a message. Both independent sends remain visible.
  assert.equal(
    outgoingTranscript([pending], [receipt, { ...receipt, id: "two" }]).items
      .length,
    2,
  );
  const batch: Message[] = [
    { id: "lead:one", clientMessageId: "one", role: "user", text: "Same text" },
    { id: "lead:two", clientMessageId: "two", role: "user", text: "Same text" },
  ];
  // Removed inference: transcript presence without a status does not retire the receipt.
  assert.deepEqual(
    outgoingTranscript(batch, [receipt, { ...receipt, id: "two" }]).observed,
    [],
  );
  const edited: OutgoingMessage = {
    ...receipt,
    displayText: "Edited queue text",
  };
  assert.equal(
    required(outgoingTranscript([], [edited]).items[0]).text,
    "Edited queue text",
  );
  assert.equal(edited.body.text, "Same text");
  assert.deepEqual(
    outgoingTranscript(
      [{ id: "lead:one:0", role: "user", text: "Same text" }],
      [receipt],
    ).observed,
    [],
  );
  assert.deepEqual(
    outgoingTranscript(
      [{ id: "lead:one:1", role: "user", text: "Same text" }],
      [receipt],
    ).observed,
    [],
  );
});

it("retires delivered local cards outside the page only with exact saved input proof", () => {
  const latest: Message[] = [
    { id: "lead:new", role: "assistant", text: "Latest reply" },
  ];
  const delivered: OutgoingMessage = {
    ...receipt,

    receipt: { id: "one", status: "delivered", materialized: true },
  };
  const result = outgoingTranscript(latest, [delivered]);
  assert.deepEqual(result.items, latest);
  assert.deepEqual(result.observed, ["one"]);
  for (const materialized of [false, undefined] as const) {
    const unproven = {
      ...delivered,
      receipt: { ...required(delivered.receipt), materialized },
    };
    assert.equal(outgoingTranscript(latest, [unproven]).items.length, 2);
    assert.deepEqual(outgoingTranscript(latest, [unproven]).observed, []);
  }
  for (const status of ["queued", "sending", "uncertain", "failed"] as const) {
    assert.equal(
      outgoingTranscript(latest, [
        { ...delivered, status: { source: "server", status } },
      ]).items.length,
      2,
    );
  }
  for (const status of ["queued", "sending", "sending", "uncertain"] as const) {
    assert.equal(
      outgoingTranscript(latest, [
        { ...delivered, receipt: { ...required(delivered.receipt), status } },
      ]).items.length,
      2,
    );
  }
  const original: Message = {
    id: "lead:batch-second",
    clientMessageId: "one",
    role: "user",
    text: "Same text",
    materialized: true,
  };
  assert.deepEqual(outgoingTranscript([original], [delivered]).items, [
    { ...original, deliveryStatus: delivered.status, deliveryError: undefined },
  ]);
  assert.deepEqual(outgoingTranscript([original], [delivered]).observed, []);
});

it("normalizes legacy next-turn rows without rewriting their retry payload", () => {
  const legacy: OutgoingMessage = {
    ...receipt,
    body: { ...receipt.body, delivery: DeliveryMomentInput.after_turn },
  };
  const row = required(outgoingTranscript([], [legacy]).items[0]);
  assert.equal(row.requestedDelivery, "next_turn");
  assert.equal(legacy.body.delivery, "after_turn");
  assert.equal(
    required(outgoingTranscript([], [receipt]).items[0]).requestedDelivery,
    "current_turn",
  );
  for (const delivery of ["later", 1, false] as const) {
    assert.throws(
      () =>
        outgoingTranscript(
          [],
          // @ts-expect-error The delivery field deliberately contains an unsupported string, number, or boolean.
          [{ ...receipt, body: { ...receipt.body, delivery } }],
        ),
      /Invalid delivery moment/,
    );
  }
});

it("uses the status alone to identify queued and dispatched messages", () => {
  const message = componentMessage({
    deliveryStatus: { source: "server", status: "queued" },
  });
  assert.equal(explicitQueue(message), true);
  assert.equal(dispatchedMessage(message), false);
  for (const deliveryStatus of [
    "sending",
    "delivered",
    "failed",
    "cancelled",
    "uncertain",
    "stored_only",
  ] as const) {
    assert.equal(
      explicitQueue({
        ...message,
        deliveryStatus: { source: "server", status: deliveryStatus },
      }),
      false,
    );
    assert.equal(
      dispatchedMessage({
        ...message,
        deliveryStatus: { source: "server", status: deliveryStatus },
      }),
      true,
    );
  }
});

it("applies queue permutations and rejects duplicate or incomplete orders", () => {
  assert.deepEqual(
    mergeQueueOrder(["normal", "a", "hidden", "b"], ["a", "b"], ["b", "a"]),
    ["normal", "b", "hidden", "a"],
  );
  assert.throws(() =>
    mergeQueueOrder(["normal", "a", "b"], ["a", "b"], ["a", "a"]),
  );
  assert.throws(() => mergeQueueOrder(["normal", "a", "b"], ["a", "b"], ["a"]));
  assert.throws(() =>
    mergeQueueOrder(["normal", "a", "b"], ["a", "b"], ["a", "missing"]),
  );
  assert.throws(() =>
    mergeQueueOrder(["normal", "a", "b"], ["a", "missing"], ["a", "missing"]),
  );
  assert.throws(() => mergeQueueOrder(["normal", "a"], ["a", "a"], ["a"]), {
    message: "The queue changed. Reload before reordering.",
  });
  assert.throws(
    () => mergeQueueOrder(["normal", "a", "b"], ["a", "b"], ["a", "missing"]),
    { message: "The queue changed. Reload before reordering." },
  );
  assert.deepEqual(
    mergeQueueOrder(["normal", "a", "b"], ["a", "b"], ["b", "a"]),
    ["normal", "b", "a"],
  );
});

it("uses stable render and receipt identities rather than message text", () => {
  assert.equal(
    messageRenderKey(
      componentMessage({
        id: "server-row",
        role: "user",
        clientMessageId: "send-1",
      }),
    ),
    "client:send-1",
  );
  assert.equal(
    messageRenderKey(
      componentMessage({
        id: "assistant-row",
        role: "assistant",
        clientMessageId: "send-1",
      }),
    ),
    "assistant-row",
  );
  assert.equal(
    messageRenderKey(componentMessage({ id: "user-row", role: "user" })),
    "user-row",
  );

  assert.equal(
    receiptMessage(
      componentMessage({ id: "server-row", clientMessageId: "one" }),
      receipt,
    ),
    true,
  );
  assert.equal(receiptMessage(componentMessage({ id: "one" }), receipt), true);
  assert.equal(
    receiptMessage(componentMessage({ id: "lead:one" }), receipt),
    true,
  );
  assert.equal(
    receiptMessage(componentMessage({ id: "lead:one:0" }), receipt),
    true,
  );
  assert.equal(
    receiptMessage(componentMessage({ id: "lead:one:1" }), receipt),
    false,
  );
  assert.equal(
    receiptMessage(
      componentMessage({ id: "other:one:0", clientMessageId: "different" }),
      receipt,
    ),
    false,
  );
});

it("updates a matching receipt without replacing existing transcript content", () => {
  const durable: Message = {
    id: "lead:one",
    clientMessageId: "one",
    role: "user",
    text: "server text",
    assets: [componentAttachment({ id: "server-asset" })],
    materialized: true,
    requestedDelivery: DeliveryMoment.next_turn,
    deliveryStatus: { source: "server", status: MessageSendStatus.delivered },
  };
  const entry: OutgoingMessage = {
    ...receipt,
    body: { ...receipt.body, delivery: DeliveryMomentInput.after_tool },
    attachments: [componentAttachment({ id: "outbox-asset" })],
    status: { source: "server", status: MessageSendStatus.failed },
    receipt: { id: "one", status: "failed", error: "late receipt error" },
  };
  const result = outgoingTranscript([durable], [entry]);
  assert.deepEqual(result.observed, ["one"]);
  assert.equal(result.items.length, 1);
  assert.equal(required(result.items[0]).text, "server text");
  assert.deepEqual(required(result.items[0]).assets, [
    componentAttachment({ id: "server-asset" }),
  ]);
  assert.equal(required(result.items[0]).requestedDelivery, "next_turn");
  // Removed inference: a delivered transcript no longer overrides the current server receipt.
  assert.deepEqual(required(result.items[0]).deliveryStatus, entry.status);
  assert.equal(required(result.items[0]).deliveryError, "late receipt error");

  const emptyDurable: Message = {
    ...durable,
    id: "lead:two",
    clientMessageId: "two",
    assets: undefined,
  };
  const noAttachmentChange = outgoingTranscript(
    [emptyDurable],
    [
      {
        ...receipt,
        id: "two",
        attachments: [componentAttachment({ id: "outbox-only" })],
      },
    ],
  );
  assert.equal(required(noAttachmentChange.items[0]).assets, undefined);
  assert.deepEqual(
    required(noAttachmentChange.items[0]).deliveryStatus,
    receipt.status,
  );
});

it("restores attachments and requested delivery to a local transcript row", () => {
  const materializing: Message = {
    id: "lead:one",
    clientMessageId: "one",
    role: "user",
    text: "Same text",
    materialized: false,
  };
  const result = outgoingTranscript(
    [materializing],
    [
      {
        ...receipt,
        body: { ...receipt.body, delivery: DeliveryMomentInput.after_tool },
        attachments: [componentAttachment({ id: "asset-one" })],
      },
    ],
  );
  assert.deepEqual(required(result.items[0]).assets, [
    componentAttachment({ id: "asset-one" }),
  ]);
  assert.equal(required(result.items[0]).requestedDelivery, "current_turn");
  assert.deepEqual(result.observed, []);

  const existingDelivery: Message = {
    ...materializing,
    requestedDelivery: DeliveryMoment.next_turn,
  };
  const unchangedDelivery = outgoingTranscript(
    [existingDelivery],
    [
      {
        ...receipt,
        body: { ...receipt.body, delivery: DeliveryMomentInput.after_tool },
      },
    ],
  );
  assert.equal(
    required(unchangedDelivery.items[0]).requestedDelivery,
    "next_turn",
  );
  assert.deepEqual(
    required(unchangedDelivery.items[0]).deliveryStatus,
    receipt.status,
  );
});

it("shows failed and uncertain receipts until the transcript confirms their outcome", () => {
  for (const status of ["failed", "uncertain"] as const) {
    const present: Message = {
      id: "lead:one",
      clientMessageId: "one",
      role: "user",
      text: "Same text",
    };
    const result = outgoingTranscript(
      [present],
      [
        {
          ...receipt,
          status: { source: "server", status },
          receipt: { id: "one", status, error: `${status} detail` },
        },
      ],
    );
    assert.deepEqual(required(result.items[0]).deliveryStatus, {
      source: "server",
      status,
    });
    assert.equal(required(result.items[0]).deliveryError, `${status} detail`);
    assert.deepEqual(result.observed, []);

    const alreadyLabeled: Message = {
      ...present,
      deliveryStatus: { source: "server", status: MessageSendStatus.sending },
    };
    const preserved = outgoingTranscript(
      [alreadyLabeled],
      [
        {
          ...receipt,
          status: { source: "server", status },
          receipt: { id: "one", status, error: "must not overwrite" },
        },
      ],
    );
    // Removed inference: prior transcript status does not override a server receipt.
    assert.deepEqual(required(preserved.items[0]).deliveryStatus, {
      source: "server",
      status,
    });
    assert.equal(
      required(preserved.items[0]).deliveryError,
      "must not overwrite",
    );

    const materializedWithoutLabel: Message = {
      ...present,
      materialized: true,
      deliveryStatus: undefined,
    };
    const notYetObserved = outgoingTranscript(
      [materializedWithoutLabel],
      [
        {
          ...receipt,
          status: { source: "server", status },
          receipt: { id: "one", status, error: `${status} detail` },
        },
      ],
    );
    assert.deepEqual(notYetObserved.observed, []);
    assert.deepEqual(required(notYetObserved.items[0]).deliveryStatus, {
      source: "server",
      status,
    });
  }
});

it("creates local receipt rows with display content and accepted queue status", () => {
  const missingReceipt: OutgoingMessage = {
    ...receipt,
    status: { source: "server", status: MessageSendStatus.delivered },
    receipt: undefined,
    displayText: "Edited draft",
    attachments: undefined,
    created: 1234,

    body: {
      room: "lead",
      text: "Original text",
      delivery: DeliveryMomentInput.after_turn,
    } as OutgoingMessage["body"],
  };
  const row = required(outgoingTranscript([], [missingReceipt]).items[0]);
  assert.equal(row.id, "lead:one");
  assert.equal(row.clientMessageId, "one");
  assert.equal(row.role, "user");
  assert.equal(row.title, "You");
  assert.equal(row.text, "Edited draft");
  assert.deepEqual(row.assets, []);
  assert.equal(row.at, 1.234);
  assert.deepEqual(row.deliveryStatus, {
    source: "server",
    status: "delivered",
  });
  assert.equal(row.deliveryError, undefined);
  assert.equal(row.localDelivery, true);
  assert.equal(row.requestedDelivery, "next_turn");

  const queued = outgoingTranscript(
    [],
    [
      {
        ...receipt,
        receipt: { status: MessageSendStatus.sending } as NonNullable<
          OutgoingMessage["receipt"]
        >,
      },
    ],
  ).items[0];
  assert.deepEqual(required(queued).deliveryStatus, {
    source: "server",
    status: "delivered",
  });
  const waiting = outgoingTranscript(
    [],
    [
      {
        ...receipt,
        receipt: { status: MessageSendStatus.queued } as NonNullable<
          OutgoingMessage["receipt"]
        >,
      },
    ],
  ).items[0];
  assert.deepEqual(required(waiting).deliveryStatus, {
    source: "server",
    status: "delivered",
  });
  const missing = required(
    outgoingTranscript([], [{ ...receipt, receipt: undefined }]).items[0],
  );
  assert.deepEqual(missing.deliveryStatus, {
    source: "server",
    status: "delivered",
  });
  const nonAccepted = outgoingTranscript(
    [],
    [
      {
        ...receipt,
        status: { source: "server", status: MessageSendStatus.queued },
        receipt: { status: MessageSendStatus.queued } as NonNullable<
          OutgoingMessage["receipt"]
        >,
      },
    ],
  ).items[0];
  assert.deepEqual(required(nonAccepted).deliveryStatus, {
    source: "server",
    status: "queued",
  });
});

it("labels every generated message send status", () => {
  const labels = {
    queued: "Queued",
    sending: "Sending…",
    delivered: "Sent",
    uncertain: "Delivery unconfirmed",
    failed: "Not sent",
    cancelled: "Cancelled",
    stored_only: "Stored",
  } satisfies Record<import("../../generated/api").MessageSendStatus, string>;
  for (const [status, label] of Object.entries(labels) as [
    keyof typeof labels,
    string,
  ][])
    assert.equal(
      deliveryLabel(
        componentMessage({ deliveryStatus: { source: "server", status } }),
      ),
      label,
    );
  const localLabels = {
    stored: "Stored on device",
    in_flight: "Sending…",
    transport_failed: "Could not contact server",
    receipt_expired: "Delivery outcome unknown",
    uncertain: "Delivery unconfirmed",
    removed: "Removed from device",
  } as const;
  for (const [kind, label] of Object.entries(localLabels)) {
    const state =
      kind === "transport_failed" ||
      kind === "receipt_expired" ||
      kind === "uncertain"
        ? ({ kind, error: "offline" } as const)
        : { kind: kind as "stored" | "in_flight" | "removed" };
    assert.equal(
      deliveryLabel(
        componentMessage({ deliveryStatus: { source: "device", state } }),
      ),
      label,
    );
  }
  assert.equal(deliveryLabel(componentMessage({})), "");
  // @ts-expect-error Unknown delivery statuses are rejected by the generated contract.
  const unknown: import("../../generated/api").MessageSendStatus =
    "future-status";
  void unknown;
});

// The App merges the durable outbox with this tab's send copies. The chat then
// renders local cards; OutboxControls shows retry actions for queued entries.
const steer: OutgoingMessage["body"] = {
  id: "steer",
  room: "lead",
  text: "Steer the turn",
  assets: [],
  delivery: DeliveryMomentInput.current_turn,
};
function localCards(
  outbox: OutboxEntry[],
  local: Record<string, OutgoingMessage>,
  observed = new Set<string>(),
  history: Message[] = [],
) {
  const shown = mergeOutgoing(outbox, local, observed).filter(
    (entry) => entry.body.room === "lead",
  );
  return outgoingTranscript(history, shown)
    .items.filter((item) => item.localDelivery)
    .map((item) => ({
      label: deliveryLabel(item),
      retryControls:
        shown.find((entry) => entry.id === item.clientMessageId)?.status
          .source === "device",
    }));
}

it("hides a delivered message whose first send result was queued after a chat switch", () => {
  const sending: OutgoingMessage = {
    id: "steer",
    body: steer,
    status: { source: "device", state: { kind: "stored" } },
    created: 1,
    attachments: [],
  };
  assert.deepEqual(localCards([], { steer: sending }), [
    { label: "Stored on device", retryControls: true },
  ]);

  // The first response is lost, so the send result stays queued for retry.
  const local: Record<string, OutgoingMessage> = {
    steer: {
      ...sending,
      status: {
        source: "device",
        state: { kind: "transport_failed", error: "Timed out" },
      },
    },
  };
  const stored: OutboxEntry = {
    id: "steer",
    body: steer,
    status: { source: "device", state: { kind: "stored" } },

    created: 1,

    attachments: [],
  };
  assert.deepEqual(localCards([stored], local), [
    { label: "Stored on device", retryControls: true },
  ]);

  // Another chat is open. The outbox retry delivers the message and receipt
  // recovery confirms the saved transcript input. No chat observes it.
  const delivered: OutboxEntry = {
    ...stored,
    status: { source: "server", status: MessageSendStatus.delivered },

    receipt: { id: "steer", status: "delivered", materialized: true },
  };

  // Back in the chat, the transcript page does not contain the input yet.
  assert.deepEqual(localCards([delivered], local), []);
});

it("lets the durable outbox state replace an in-flight send copy", () => {
  const sending: OutgoingMessage = {
    id: "steer",
    body: steer,
    status: { source: "device", state: { kind: "stored" } },
    created: 1,
    attachments: [],
  };
  const stored: OutboxEntry = {
    id: "steer",
    body: steer,
    status: { source: "device", state: { kind: "stored" } },

    created: 1,

    attachments: [],
  };
  // The outbox has not resolved the request yet, so the copy reports it.
  assert.deepEqual(localCards([stored], { steer: sending }), [
    { label: "Stored on device", retryControls: true },
  ]);
  // A server receipt takes precedence over a device fact for the same command.
  assert.deepEqual(
    localCards([stored], {
      steer: { ...sending, status: { source: "server", status: "queued" } },
    }),
    [{ label: "Queued", retryControls: false }],
  );
  // A rejected send that the outbox never stored keeps its local failure.
  assert.deepEqual(
    localCards([], {
      steer: {
        ...sending,
        status: {
          source: "device",
          state: { kind: "transport_failed", error: "No" },
        },
      },
    }),
    [{ label: "Could not contact server", retryControls: true }],
  );
  // A stored uncertain outcome stays uncertain while a repeat send of the same
  // identity is in flight. The outbox does not send it again.
  for (const copy of ["sending", "queued", "delivered"] as const)
    assert.deepEqual(
      localCards(
        [
          {
            ...stored,
            status: { source: "server", status: MessageSendStatus.uncertain },
          },
        ],
        { steer: { ...sending, status: { source: "server", status: copy } } },
      ),
      [{ label: "Delivery unconfirmed", retryControls: false }],
    );
  // A confirmed outbox result is not shown again as an earlier send state.
  for (const copy of ["sending", "queued"] as const)
    assert.deepEqual(
      localCards(
        [
          {
            ...stored,
            status: { source: "server", status: MessageSendStatus.delivered },

            receipt: { id: "steer", status: "delivered", materialized: true },
          },
        ],
        { steer: { ...sending, status: { source: "server", status: copy } } },
      ),
      [],
    );
  // An observed send stays hidden whatever its local state is.
  assert.deepEqual(
    localCards([stored], { steer: sending }, new Set(["steer"])),
    [],
  );
});

it("keeps a stored request visible when a cached projection marks it observed", () => {
  assert.equal(mergeOutgoing([receipt], {}, new Set([receipt.id])).length, 1);
});

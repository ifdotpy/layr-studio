import { MessageSendStatus } from "../generated/api.ts";
import { required } from "../components/componentTestFixtures.ts";
import type { Message } from "../types";
import type { OutgoingMessage } from "./send";
import type { MessageReceipt } from "./messageReceipts";
import assert from "node:assert/strict";
import {
  checkedMessageReceipts,
  receiptOutgoing,
  receiptTranscript,
} from "./messageReceipts.ts";

import { it } from "vitest";

it("rejects receipt responses from another workspace", () => {
  for (const workspaceId of ["foreign", undefined])
    assert.throws(
      () =>
        checkedMessageReceipts(
          {
            agent: "chat",
            workspaceId,
            items: [
              {
                id: "one",
                status: MessageSendStatus.delivered,
                materialized: true,
              },
            ],
          },
          "chat",
          ["one"],
          "workspace",
        ),
      { message: "The delivery receipts belong to another workspace." },
    );
});

it("joins outgoing messages to accepted and delivered receipts by identity", () => {
  const delivered = {
    id: "sso",
    status: MessageSendStatus.delivered,
  } satisfies MessageReceipt;
  const accepted = {
    id: "sso",
    body: { id: "sso", room: "chat", text: "sso done" },
    created: 1,
    status: { source: "server", status: MessageSendStatus.delivered },
    receipt: { id: "sso", status: "queued" },
  } satisfies OutgoingMessage;
  assert.deepEqual(
    checkedMessageReceipts({ agent: "chat", items: [delivered] }, "chat", [
      "sso",
    ]),
    [delivered],
  );
  for (const status of [
    "queued",
    "queued",
    "sending",
    "sending",
    "delivered",
    "delivered",
    "delivered",
    "uncertain",
    "failed",
    "cancelled",
    "stored_only",
  ] as const)
    assert.deepEqual(
      checkedMessageReceipts(
        { agent: "chat", items: [{ id: "sso", status }] },
        "chat",
        ["sso"],
      ),
      [{ id: "sso", status }],
    );
  assert.throws(
    () =>
      checkedMessageReceipts({ agent: "foreign", items: [delivered] }, "chat", [
        "sso",
      ]),
    { message: "The delivery receipts belong to another chat." },
  );
  for (const value of [
    {
      agent: "chat",
      items: [{ id: "other", status: MessageSendStatus.delivered }],
    },
    { agent: "chat", items: [delivered, delivered] },
  ] satisfies Parameters<typeof checkedMessageReceipts>[0][])
    assert.throws(() => checkedMessageReceipts(value, "chat", ["sso"]), {
      message: "The delivery receipts do not match the requested messages.",
    });
  assert.throws(
    () =>
      checkedMessageReceipts(
        {
          agent: "chat",
          items: [
            { id: "first", status: MessageSendStatus.queued },
            { id: "unexpected", status: MessageSendStatus.delivered },
          ],
        },
        "chat",
        ["first"],
      ),
    { message: "The delivery receipts do not match the requested messages." },
  );
  assert.deepEqual(
    checkedMessageReceipts(
      {
        agent: "chat",
        items: [
          { id: "first", status: MessageSendStatus.queued },
          { id: "second", status: MessageSendStatus.delivered },
        ],
      },
      "chat",
      ["first", "second"],
    ),
    [
      { id: "first", status: "queued" },
      { id: "second", status: "delivered" },
    ],
  );
  assert.ok(receiptOutgoing);
  assert.deepEqual(
    receiptOutgoing(accepted, delivered).receipt!.status,
    "delivered",
  );
  assert.deepEqual(receiptOutgoing(accepted, delivered), {
    ...accepted,
    status: { source: "server", status: "delivered" },
    receipt: delivered,
  });
  assert.deepEqual(
    receiptOutgoing(
      {
        ...accepted,
        status: { source: "server", status: MessageSendStatus.uncertain },
      },
      delivered,
    ).status,
    { source: "server", status: "delivered" },
    "A receipt resolves an uncertain send using the same message identity",
  );
  assert.deepEqual(
    receiptOutgoing(accepted, {
      id: "sso",
      status: "failed",
      error: "server rejected the message",
    }).receipt?.error,
    "server rejected the message",
  );
  assert.deepEqual(
    receiptOutgoing(
      {
        ...accepted,
        status: { source: "server", status: MessageSendStatus.queued },
      },
      delivered,
    ).status,
    { source: "server", status: "delivered" },
    "A matching server receipt confirms delivery",
  );
  // Removed inference: the client no longer retains an earlier delivered status over a newer receipt.
  assert.deepEqual(
    receiptOutgoing(accepted, { id: "sso", status: "queued" }).status,
    { source: "server", status: "queued" },
  );
  const pending = {
    id: "chat:sso",
    clientMessageId: "sso",
    role: "user",
    text: "sso done",
    materialized: false,
  } satisfies Message;
  const shown = receiptTranscript(
    [pending],
    "chat",
    new Map([["sso", delivered]]),
  )[0];
  assert.ok(shown);
  const removed: Message = {
    ...pending,
    deliveryStatus: { source: "device", state: { kind: "removed" } },
  };
  assert.equal(receiptTranscript([removed], "chat", new Map())[0], removed);
  assert.deepEqual(
    receiptTranscript(
      [removed],
      "chat",
      new Map([["sso", { id: "sso", status: "cancelled" }]]),
    )[0]?.deliveryStatus,
    { source: "server", status: "cancelled" },
  );
  assert.deepEqual(shown.deliveryStatus, {
    source: "server",
    status: "delivered",
  });

  assert.equal(
    shown.materialized,
    false,
    "A receipt does not fabricate a transcript item",
  );
  assert.equal(
    receiptTranscript([pending], "chat", new Map())[0],
    pending,
    "An omitted ID does not prove success",
  );
  const fallbackId: Message = {
    ...pending,
    id: "chat:fallback",
    clientMessageId: undefined,
  };
  assert.deepEqual(
    receiptTranscript(
      [fallbackId],
      "chat",
      new Map([
        ["fallback", { id: "fallback", status: "failed", error: "no" }],
      ]),
    )[0],
    {
      ...fallbackId,
      deliveryStatus: { source: "server", status: "failed" },
      deliveryError: "no",
    },
  );
  const nonPrefixedId: Message = {
    ...pending,
    id: "legacy-id",
    clientMessageId: undefined,
  };
  assert.deepEqual(
    receiptTranscript(
      [nonPrefixedId],
      "chat",
      new Map([["legacy-id", { id: "legacy-id", status: "queued" }]]),
    )[0],
    {
      ...nonPrefixedId,
      deliveryStatus: { source: "server", status: "queued" },
      deliveryError: undefined,
    },
  );
  const roomPrefixWithoutDelimiter: Message = {
    ...pending,
    id: "chatty",
    clientMessageId: undefined,
  };
  assert.ok(receiptTranscript);
  assert.deepEqual(
    required(
      receiptTranscript(
        [roomPrefixWithoutDelimiter],
        "chat",
        new Map([["chatty", { id: "chatty", status: "delivered" }]]),
      )[0],
    ).deliveryStatus,
    { source: "server", status: "delivered" },
    "Only the exact room prefix followed by a colon is stripped",
  );
  const preferredClientId = {
    ...pending,
    id: "chat:other",
    clientMessageId: "sso",
  };

  assert.deepEqual(
    required(
      receiptTranscript(
        [preferredClientId],
        "chat",
        new Map([
          ["sso", { id: "sso", status: "failed" }],
          ["other", { id: "other", status: "cancelled" }],
        ]),
      )[0],
    ).deliveryStatus,
    { source: "server", status: "failed" },
    "The stable client message ID takes precedence over the transcript ID",
  );
  const assistant = { ...pending, role: "assistant" } satisfies Message;
  assert.equal(
    receiptTranscript([assistant], "chat", new Map([["sso", delivered]]))[0],
    assistant,
    "Receipts update user messages only",
  );
  const alreadyDelivered = {
    ...pending,
    deliveryStatus: { source: "server", status: MessageSendStatus.delivered },
    deliveryError: "stale",
  } satisfies Message;
  assert.deepEqual(
    receiptTranscript(
      [alreadyDelivered],
      "chat",
      new Map([["sso", { id: "sso", status: "queued", error: "old" }]]),
    )[0],
    {
      ...alreadyDelivered,
      deliveryStatus: { source: "server", status: "queued" },
      deliveryError: "old",
    },
  );
  for (const status of ["uncertain", "failed", "cancelled"] as const)
    assert.deepEqual(receiptOutgoing(accepted, { id: "sso", status }).status, {
      source: "server",
      status,
    });
  console.log("PASS exact identity and receipt status");
});

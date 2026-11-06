import { describe, expect, it } from "vitest";
import { isUnsentRecord, UNSENT_FORMAT_VERSION as version } from "./schema";

// The stored unsent records keep one validation contract, whatever zod build
// checks them.
const draft = { id: "d", payload: "{}", seq: 3, _deleted: false };
const blob = (size: number) => new Blob([new Uint8Array(size)]);
const upload = {
  id: "u",
  workspace: "w",
  stateDir: "s",
  agent: "a",
  name: "n.txt",
  size: 4,
  mime: "text/plain",
  created: 1,
  bytes: blob(4),
};
const send = {
  request: {
    id: "r",
    client: "c",
    number: 1,
    kind: "send",
    target: "t",
    body: { text: "hello" },
  },
  localState: { kind: "transport_failed", error: "offline" },
  workspaceId: "w",
  created: 1,
  attachments: [],
};

describe("unsent record validation", () => {
  it.each([
    ["text", { version, kind: "text", value: "draft text" }],
    ["draft", { version, kind: "draft", value: draft }],
    ["upload", { version, kind: "upload", value: upload }],
    [
      "record",
      {
        version,
        kind: "record",
        value: {
          id: "r",
          chatId: "c",
          created: 1,
          sampleRate: 16000,
          samples: 10,
          state: "ready",
          transcript: "words",
        },
      },
    ],
    ["chunk", { version, kind: "chunk", id: "r", index: 0, value: blob(2) }],
    ["send", { version, kind: "send", value: send }],
  ])("accepts a %s record", (_name, value) => {
    expect(isUnsentRecord(value)).toBe(true);
  });

  it.each([
    ["another version", { version: 2, kind: "text", value: "x" }],
    ["an extra key", { version, kind: "text", value: "x", extra: 1 }],
    [
      "a fractional sequence",
      { version, kind: "draft", value: { ...draft, seq: 1.5 } },
    ],
    [
      "an unsafe sequence",
      {
        version,
        kind: "draft",
        value: { ...draft, seq: Number.MAX_SAFE_INTEGER + 2 },
      },
    ],
    [
      "a negative chunk index",
      { version, kind: "chunk", id: "r", index: -1, value: blob(2) },
    ],
    [
      "an odd chunk size",
      { version, kind: "chunk", id: "r", index: 0, value: blob(3) },
    ],
    [
      "an upload size mismatch",
      { version, kind: "upload", value: { ...upload, size: 5 } },
    ],
    [
      "an unknown send state",
      {
        version,
        kind: "send",
        value: { ...send, localState: { kind: "sent" } },
      },
    ],
    [
      "an unknown delivery moment",
      {
        version,
        kind: "send",
        value: {
          ...send,
          request: { ...send.request, body: { text: "x", delivery: "never" } },
        },
      },
    ],
  ])("rejects %s", (_name, value) => {
    expect(isUnsentRecord(value)).toBe(false);
  });
});

import assert from "node:assert/strict";
import { it } from "vitest";
import type { Message } from "../../../../types.ts";
import { providerUnknown } from "./ProviderUnknownItem.tsx";

function item(extra: Partial<Message> = {}): Message {
  return {
    id: "i",
    role: "output",
    title: "futureItem",
    text: "{}",
    ...extra,
  } as Message;
}

it("reads the kind and raw provider item of an unknown item", () => {
  const raw = '{"id":"i","type":"futureItem","payload":{"n":1}}';
  assert.deepEqual(
    providerUnknown(item({ providerUnknown: { kind: "futureItem", raw } })),
    { kind: "futureItem", raw },
  );
});

it("returns nothing for items without the marker", () => {
  assert.equal(providerUnknown(item()), undefined);
});

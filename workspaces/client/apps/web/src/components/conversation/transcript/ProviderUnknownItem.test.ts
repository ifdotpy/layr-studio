import assert from "node:assert/strict";
import { it } from "vitest";
import type { Message } from "../../../types";
import { providerUnknown } from "./ProviderUnknownItem.tsx";

const base = { id: "i", role: "output", title: "futureItem", text: "{}" };

it("reads the kind and raw provider item of an unknown item", () => {
  const raw = { id: "i", type: "futureItem", payload: { n: 1 } };
  const item = {
    ...base,
    providerUnknown: { kind: "futureItem", raw },
  } as unknown as Message;
  assert.deepEqual(providerUnknown(item), { kind: "futureItem", raw });
});

it("ignores items without a valid unknown marker", () => {
  assert.equal(providerUnknown(base as unknown as Message), undefined);
  const noKind = {
    ...base,
    providerUnknown: { raw: {} },
  } as unknown as Message;
  assert.equal(providerUnknown(noKind), undefined);
  const nullValue = { ...base, providerUnknown: null } as unknown as Message;
  assert.equal(providerUnknown(nullValue), undefined);
});

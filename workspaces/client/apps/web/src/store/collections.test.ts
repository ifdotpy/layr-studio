import { describe, expect, it } from "vitest";
import {
  ENTITY_COLLECTIONS,
  emptyEntityValues,
  setEntityValue,
} from "../generated/entityCollections.ts";
import {
  createStateMirror,
  serializeStateCollections,
} from "../generated/observableResources.ts";

describe("Rust-generated collection inventory", () => {
  it("initializes a distinct typed map for every payload collection", () => {
    const first = emptyEntityValues();
    const second = emptyEntityValues();
    expect(Object.keys(first).sort()).toEqual([...ENTITY_COLLECTIONS].sort());
    expect(new Set(ENTITY_COLLECTIONS).size).toBe(ENTITY_COLLECTIONS.length);
    for (const name of ENTITY_COLLECTIONS) {
      expect(first[name]).toBeInstanceOf(Map);
      expect(first[name].size).toBe(0);
      expect(first[name]).not.toBe(second[name]);
    }
  });
  it("serializes selected native facts without retaining mutable model references", () => {
    const mirror = createStateMirror();
    mirror.apply([
      {
        collection: "agentTail",
        id: "lead",
        value: { id: "lead", tail: "before" },
      },
    ]);
    const snapshot = serializeStateCollections(mirror.current);
    expect(snapshot).toEqual([
      {
        collection: "agentTail",
        id: "lead",
        value: { id: "lead", tail: "before" },
      },
    ]);
    mirror.apply([
      {
        collection: "agentTail",
        id: "lead",
        value: { id: "lead", tail: "after" },
      },
    ]);
    expect(snapshot[0]?.value).toEqual({ id: "lead", tail: "before" });
    mirror.apply([]);
    expect(serializeStateCollections(mirror.current)).toEqual([]);
  });
});

it("dispatches typed payloads into their generated collection maps", () => {
  const values = emptyEntityValues();
  setEntityValue(values, {
    collection: "agentTail",
    id: "lead",
    value: { id: "lead", tail: "text" },
  });
  expect(values.agentTail.get("lead")).toEqual({ id: "lead", tail: "text" });
  expect(values.agent.size).toBe(0);
});

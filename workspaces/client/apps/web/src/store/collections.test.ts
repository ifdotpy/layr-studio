import {
  StudioErrorLevel,
  StudioErrorEffectCertainty,
} from "../generated/api.ts";
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

it("generates account summaries, model rows and rate limits on the shared state resource", () => {
  const state = createStateMirror().current;
  for (const name of [
    "accountsSummary",
    "modelsByAccount",
    "rateLimitsByAccount",
  ] as const) {
    expect(state[name]).toBeDefined();
    expect(state[name].size).toBe(0);
  }
});

it("keeps error action unions atomic and preserves equal native facts", () => {
  const mirror = createStateMirror();
  const error = {
    id: "turn",
    kind: "turn_disconnected",
    level: StudioErrorLevel.error,
    at: 1,
    reason: "Disconnected",
    detail: "",
    subject: "",
    diagnostics: {
      context: { source: "agent", turnId: "turn", epoch: 0 },
      certainty: StudioErrorEffectCertainty.unknown,
      causes: [],
    },
    action: "resume",
    agentId: "lead",
    agentName: "Lead",
  } satisfies import("../generated/api.ts").components["schemas"]["StudioError"];
  mirror.apply([{ collection: "studioError", id: error.id, value: error }]);
  const first = mirror.current.studioErrors.get(error.id);
  mirror.apply([
    { collection: "studioError", id: error.id, value: structuredClone(error) },
  ]);
  expect(mirror.current.studioErrors.get(error.id)).toBe(first);
  mirror.apply([
    {
      collection: "studioError",
      id: error.id,
      value: { ...error, action: "details" },
    },
  ]);
  expect(mirror.current.studioErrors.get(error.id)?.action).toBe("details");
  expect(first?.action).toBe("resume");
  mirror.apply([]);
  expect(mirror.current.studioErrors.size).toBe(0);
});

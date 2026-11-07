import { expect, it } from "vitest";
import { agentId } from "../generated/observableResources.ts";
import type { ObservableResources } from "../generated/observableResources.ts";

/** Compiler witnesses exercise exactly the facade consumed by components. */
function readonlyFacade(store: ObservableResources) {
  // @ts-expect-error Generated argument facades require a branded agent identity.
  store.agentDetail("worker");
  const detail = store.agentDetail(agentId("worker")).current;
  if (detail.status === "live" || detail.status === "stale") {
    // @ts-expect-error Server-owned model fields remain readonly through keyed facades.
    detail.data.overview = null;
  }
  const slot = store.state();
  // @ts-expect-error A component cannot replace a resource's state.
  slot.current = { status: "loading" };
  const current = slot.current;
  if (
    current.status === "loading" ||
    current.status === "error" ||
    current.status === "absent"
  ) {
    // @ts-expect-error Data is unreachable before a baseline or without cached data.
    void current.data;
    return;
  }
  // @ts-expect-error The collection membership belongs to the generated store.
  current.data.agents.clear();
  const row = current.data.agents.get("chat");
  if (row) {
    // @ts-expect-error Server-owned titles cannot be edited from a component.
    row.name = "Changed";
    // @ts-expect-error Nested reason objects are readonly too.
    row.operationalStatus.reason = "ready";
  }
}
it("keeps the generated public facade's compiler witnesses reachable", () => {
  expect(readonlyFacade).toBeTypeOf("function");
});

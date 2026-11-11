// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import {
  createModelCatalogFixture,
  type ModelCatalogFixtureSource,
} from "./modelCatalogFixture.tsx";
it("subscribes before render, follows readiness and releases its source", () => {
  let receive: Parameters<
    ModelCatalogFixtureSource["subscribeState"]
  >[0] = () => {};
  const dispose = vi.fn();
  const catalog = createModelCatalogFixture({
    scope: "fixture",
    read: async () => ({ data: [] }),
    subscribeState(accept) {
      receive = accept;
      return dispose;
    },
  });
  expect(catalog.ready.get()).toBe(false);
  receive({ entitiesComplete: true, runtime: { modelsByAccount: {} } });
  expect(catalog.ready.get()).toBe(true);
  receive({ entitiesComplete: false, runtime: { modelsByAccount: {} } });
  expect(catalog.ready.get()).toBe(false);
  catalog.dispose();
  expect(dispose).toHaveBeenCalledOnce();
});

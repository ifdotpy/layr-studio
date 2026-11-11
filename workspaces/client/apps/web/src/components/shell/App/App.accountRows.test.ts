// @vitest-environment happy-dom
import { autorun, isComputedProp } from "mobx";
import { expect, it } from "vitest";
import {
  conversationActionsFixture,
  observableFixture,
} from "../integration-support/observableFixture.tsx";

it("keeps the direct-screen account projection on its server owner across unrelated UI updates", () => {
  const fixture = observableFixture();
  const areas = conversationActionsFixture(() => {}, fixture.store);
  const stop = autorun(() => areas.ui.accountRows);
  try {
    expect(isComputedProp(areas.ui, "accountRows")).toBe(true);
    const rows = areas.ui.accountRows;
    expect(rows).toEqual([]);
    areas.ui.setTeamOpen(true);
    expect(areas.ui.accountRows).toBe(rows);
    areas.ui.setTeamOpen(false);
    expect(areas.ui.accountRows).toBe(rows);
  } finally {
    stop();
    areas.dispose();
    areas.scope.services.close();
    fixture.store.dispose();
  }
});

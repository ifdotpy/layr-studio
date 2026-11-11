// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import {
  conversationActionsFixture,
  observableFixture,
} from "../../shell/integration-support/observableFixture.tsx";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";

it("settings owns one backup store for the selected chat and retires the previous chat", () => {
  const f = observableFixture([
    agentSummaryFixture("first"),
    agentSummaryFixture("second"),
  ]);
  const areas = conversationActionsFixture(() => {}, f.store);
  areas.navigation.selectExternal("first", { requestId: "first" });
  const first = areas.settingsModels.fileBackups;
  expect(first?.agentId).toBe("first");
  expect(areas.settingsModels.fileBackups).toBe(first);
  areas.navigation.selectExternal("second", { requestId: "second" });
  expect(first?.active).toBe(false);
  expect(areas.settingsModels.fileBackups?.agentId).toBe("second");
  areas.dispose();
  areas.scope.services.close();
  f.store.dispose();
});

it("model commands open the existing settings owner without a component effect", () => {
  const f = observableFixture([agentSummaryFixture("first", { isLead: true })]);
  const areas = conversationActionsFixture(() => {}, f.store);
  areas.agentsUi.openSettings("first");
  expect(
    areas.agentsUi.cell(JSON.stringify(["first", "model"]), "opened")[0],
  ).toBe(true);
  areas.agentsUi.set(JSON.stringify(["first", "model"]), "opened", false);
  areas.agentsUi.openSettings("first");
  expect(
    areas.agentsUi.cell(JSON.stringify(["first", "model"]), "opened")[0],
  ).toBe(true);
  areas.dispose();
  areas.scope.services.close();
  f.store.dispose();
});

it("project peers form is allocated at open and replaced for another project", () => {
  const f = observableFixture();
  const areas = conversationActionsFixture(() => {}, f.store);
  vi.spyOn(areas.scope.services.api, "get").mockImplementation(
    () => new Promise(() => {}),
  );
  const first = areas.settings.openProjectPeers(
    "first",
    areas.scope.services.api,
    vi.fn(),
  );
  expect(areas.settings.projectPeers).toBe(first);
  expect(
    areas.settings.openProjectPeers(
      "second",
      areas.scope.services.api,
      vi.fn(),
    ),
  ).not.toBe(first);
  areas.dispose();
  areas.scope.services.close();
  f.store.dispose();
});

it("navigation closes the old model menu and resets its role while retaining uncertain edits", () => {
  const f = observableFixture([
    agentSummaryFixture("first", { isLead: true }),
    agentSummaryFixture("second", { isLead: true }),
  ]);
  const areas = conversationActionsFixture(() => {}, f.store);
  try {
    areas.navigation.selectExternal("first", { requestId: "first" });
    const key = JSON.stringify(["first", "model"]);
    areas.agentsUi.openSettings("first");
    areas.agentsUi.set(key, "role", "review");
    const uncertain = { id: "first", model: "saved-request" };
    areas.agentsUi.set(key, "unconfirmed", uncertain);
    areas.navigation.selectExternal("second", { requestId: "second" });
    areas.navigation.selectExternal("first", { requestId: "return" });
    expect(areas.agentsUi.cell(key, "opened")[0]).toBe(false);
    expect(areas.agentsUi.cell(key, "role")[0]).toBeNull();
    expect(areas.agentsUi.cell(key, "unconfirmed")[0]).toBe(uncertain);
  } finally {
    areas.dispose();
    areas.scope.services.close();
    f.store.dispose();
  }
});

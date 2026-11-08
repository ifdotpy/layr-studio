import { setStartupFixture } from "../../components/shell/integration-support/observableFixture.tsx";
// @vitest-environment happy-dom
import { intercept, runInAction } from "mobx";
import { expect, test } from "vitest";
import { AgentSource } from "../../generated/api.ts";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";
import { componentSnapshot as snapshotFixture } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { shellFixture } from "./shell.fixture.ts";
function fixture(mobile = false) {
  const f = shellFixture(mobile);
  return { ...f, owner: f.areas.navigation, startup: f.areas.startup };
}
test("restores desktop selection and keeps mobile empty until explicit selection", () => {
  const desktop = fixture();
  desktop.services.api.save("codex-desktop-opened:workspace", "lead");
  desktop.owner.start();
  expect(desktop.owner.selectedId).toBe("lead");
  const mobile = fixture(true);
  mobile.owner.start();
  expect(mobile.owner.selectedId).toBeNull();
  desktop.owner.dispose();
  mobile.owner.dispose();
  desktop.dispose();
  mobile.dispose();
});
test("fallback handles removal and uncertain replication does not replace a confirmed selection", () => {
  const f = fixture();
  f.owner.start();
  runInAction(() => {
    f.owner.selectedId = "confirmed";
  });
  f.owner.reconcile();
  expect(f.owner.selectedId).toBe("confirmed");
  runInAction(() => {
    f.areas.creation.forget(["confirmed"]);
  });
  f.owner.reconcile();
  expect(f.owner.selectedId).toBe("lead");
  f.models.apply([]);
  expect(f.owner.selectedId).toBeNull();
  f.owner.dispose();
  f.dispose();
});
test("new jump requests preserve read-selection ordering and navigation intent", () => {
  const f = fixture();
  const model = agentSummaryFixture("lead");
  const stop = intercept(f.owner, "selectedId", (change) => {
    expect(f.areas.read.selected).toBe(change.newValue);
    return change;
  });
  f.owner.selectExternal(model.id, { messageId: "message", requestId: "one" });
  expect(f.owner.selectedJumpTarget?.requestId).toBe("one");
  stop();
  f.owner.selectExternal(model.id, { messageId: "message", requestId: "two" });
  expect(f.owner.selectedJumpTarget?.requestId).toBe("two");
  expect(f.owner.captureIntent().intent).toBe(2);
  f.dispose();
});

test("startup navigation waits in one queue and repeats message requests with fresh identities", () => {
  const f = fixture();
  runInAction(() => {
    setStartupFixture(f.areas, null);
  });
  f.owner.start();
  f.areas.ui.navigate({
    agentId: "lead",
    itemId: "message",
    requestId: "one",
  });
  expect(f.owner.selectedId).toBeNull();
  expect(f.areas.ui.opened).toBe(false);
  runInAction(() => {
    setStartupFixture(f.areas, {
      ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
      stateDir: "workspace",
    });
  });
  expect(f.areas.ui.focus).toEqual({ id: "message", requestId: "one" });
  f.areas.ui.navigate({
    agentId: "lead",
    itemId: "message",
    requestId: "two",
  });
  expect(f.areas.ui.focus).toEqual({ id: "message", requestId: "two" });
  expect(f.owner.pendingNavigation).toBeUndefined();
  f.owner.dispose();
  f.dispose();
});

test("confirmed deletion suppression refuses an explicit reopen before entity convergence", () => {
  const f = fixture();
  f.owner.start();
  f.areas.selected.suspendReaders(new Set(["lead"]));
  f.areas.finishDeletion(["lead"]);
  f.owner.selectExternal("lead", { requestId: "stale-row" });
  expect(f.owner.selectedId).not.toBe("lead");
  f.owner.dispose();
  f.dispose();
});
test("a manually closed empty mobile list stays closed through later summary changes", () => {
  const f = fixture(true);
  f.owner.start();
  f.areas.sidebar.close();
  f.models.apply([
    {
      collection: "agentSummary",
      id: "new",
      value: agentSummaryFixture("new", {
        isLead: true,
        source: AgentSource.managed,
        name: "Changed",
      }),
    },
  ]);
  expect(f.areas.sidebar.mobileOpen).toBe(false);
  f.owner.dispose();
  f.dispose();
});

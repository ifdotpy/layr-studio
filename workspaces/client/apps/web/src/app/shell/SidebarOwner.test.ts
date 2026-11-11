import { publishServerAliases } from "../../servers/serverAliases.ts";
import {
  sidebarFixtureAreas,
  sidebarFixtureRun,
} from "../../components/sidebar/Sidebar/Sidebar.fixture.tsx";
// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { reaction, runInAction } from "mobx";
import { SidebarListOwner } from "./SidebarOwner.ts";
import { observableFixture } from "../../components/shell/integration-support/observableFixture.tsx";
import { createStateMirror } from "../../generated/observableResources.ts";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";
import type { SidebarCommands } from "../../components/sidebar/support/commands.ts";

const cleanup: (() => void)[] = [];
afterEach(() =>
  cleanup
    .splice(0)
    .reverse()
    .forEach((stop) => stop()),
);

function fixture() {
  const state = observableFixture();
  const areas = sidebarFixtureAreas(state.store);
  cleanup.push(areas.dispose);
  const selection = areas.navigation;
  const visibility = areas.sidebar;
  const commands: SidebarCommands = {
    open: vi.fn(),
    onSearch() {},
    newChat() {},
    addProject() {},
    changeProject() {},
    projectAccount() {},
    async rename() {},
    remove() {},
    markUnread() {},
  };
  const receipts = createStateMirror();
  receipts.apply([
    {
      collection: "agentSummary",
      id: "new",
      value: agentSummaryFixture("new", {
        name: "Confirmed receipt",
        projectPath: "/p",
      }),
    },
  ]);
  const creation = areas.creation;
  const owner = new SidebarListOwner(
    areas,
    undefined,
    sidebarFixtureRun(areas, commands),
  );
  return { owner, state, selection, commands, visibility, creation, receipts };
}

it("reads selection from its owner and releases its selection transition on disposal", () => {
  const f = fixture();
  f.state.apply([
    {
      collection: "agentSummary",
      id: "a",
      value: agentSummaryFixture("a", { projectPath: "/p" }),
    },
  ]);
  runInAction(() => {
    f.selection.selectedId = "a";
  });
  const stop = f.owner.start();
  f.owner.setQuery("hidden");
  runInAction(() => {
    f.selection.selectedId = "a";
    f.selection.navigationIntent = 1;
  });
  expect(f.owner.selection.selectedId).toBe("a");
  expect(f.owner.query).toBe("");
  expect(f.owner.lastOpened.current).toBe("a");
  f.state.apply([
    {
      collection: "agentSummary",
      id: "ungrouped",
      value: agentSummaryFixture("ungrouped", { projectPath: "" }),
    },
  ]);
  f.owner.setQuery("hidden again");
  runInAction(() => {
    f.selection.selectedId = "ungrouped";
    f.selection.navigationIntent = 2;
  });
  expect(f.owner.query).toBe("");
  stop();
  f.owner.setQuery("retained");
  runInAction(() => {
    f.selection.navigationIntent = 3;
  });
  expect(f.owner.query).toBe("retained");
  f.state.store.dispose();
});

it("shows confirmed creations until generated state arrives, which takes precedence", () => {
  const f = fixture();
  const rows: string[][] = [];
  const stop = reaction(
    () => f.owner.model.rows.map((row) => row.agent.name || ""),
    (value) => rows.push(value),
    { fireImmediately: true },
  );
  runInAction(() => {
    f.creation.acceptCreated(
      agentSummaryFixture("new", {
        name: "Confirmed receipt",
        projectPath: "/p",
      }),
      f.creation.areas.navigation.captureIntent(),
    );
  });
  expect(f.owner.model.rows.map((row) => row.agent.name)).toEqual([
    "Confirmed receipt",
  ]);
  f.state.apply([
    {
      collection: "agentSummary",
      id: "new",
      value: agentSummaryFixture("new", {
        name: "Generated name",
        projectPath: "/p",
      }),
    },
  ]);
  expect(f.owner.model.rows).toHaveLength(1);
  expect(f.owner.model.rows[0]?.agent).toBe(
    f.state.mirror.current.agents.get("new"),
  );
  expect(rows.at(-1)).toEqual(["Generated name"]);
  stop();
  f.state.store.dispose();
});

it("groups published remote projects as read-only discovery and filters them by query", () => {
  const f = fixture();
  f.state.apply([
    {
      collection: "project",
      id: "local",
      value: { id: "local", path: "/p", name: "Local" },
    },
    {
      collection: "project",
      id: "remote",
      value: {
        id: "remote",
        path: "/remote",
        name: "Remote",
        serverLabel: "Peer device",
        homeServerId: "peer",
      },
    },
  ]);
  expect(f.owner.projects.map((project) => project.id)).toEqual(["local"]);
  expect(f.owner.remoteGroups[0]?.[1]).toBe("Peer device");
  f.owner.setQuery("missing");
  expect(f.owner.remoteGroups).toEqual([]);
  f.state.store.dispose();
});

it("reads frame aliases before start and tracks direct frame updates", () => {
  publishServerAliases({ remote: "ONE" });
  const f = fixture();
  const seen: (string | undefined)[] = [];
  const stop = reaction(
    () => f.owner.aliases.get("remote"),
    (alias) => seen.push(alias),
    { fireImmediately: true },
  );
  try {
    publishServerAliases({ remote: "TWO" });
    expect(seen).toEqual(["ONE", "TWO"]);
  } finally {
    stop();
    publishServerAliases({});
  }
});

it("reads saved compact and collapsed choices directly before start and preserves later sibling edits", () => {
  const f = fixture();
  const cache = f.owner.services.cache;
  cache.editPreference(f.owner.compactKey, {}, { project: true });
  cache.editPreference(f.owner.projectKey, {}, { folder: true });
  expect(f.owner.compactProjects).toEqual({ project: true });
  expect(f.owner.collapsed).toEqual({ folder: true });
  const seen: boolean[] = [];
  const stop = reaction(
    () => !!f.owner.collapsed.folder,
    (value) => seen.push(value),
    { fireImmediately: true },
  );
  try {
    cache.editPreference(
      f.owner.projectKey,
      { folder: true },
      { folder: false, sibling: true },
    );
    f.owner.toggleTree("folder");
    expect(f.owner.collapsed).toEqual({ folder: true, sibling: true });
    expect(seen).toEqual([true, false, true]);
    f.owner.setCompactProject("project", false);
    expect(f.owner.compactProjects).toEqual({ project: false });
  } finally {
    stop();
    cache.storage.removeItem(f.owner.compactKey);
    cache.storage.removeItem(f.owner.projectKey);
  }
});

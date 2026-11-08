import { stateFromFixture } from "./sidebarSourceFixture.ts";
import { createSidebarSourceStore } from "./sidebarSourceFixture.ts";
import { autorun } from "mobx";
import { expect, it } from "vitest";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";
import { createMergedSidebarSelector } from "./mergedSidebar.ts";
import { sidebarSnapshot, type SidebarEntity } from "./sidebarSnapshot.ts";
import { componentSnapshot } from "../components/conversation/ComposerAttachments/componentTestFixtures.ts";
const snapshotFixture = (...args: Parameters<typeof componentSnapshot>) =>
  Object.assign(componentSnapshot(...args), {
    token: "",
    stateDir: "/fixture",
  });

const facts = (name = "Original", tail = "old tail"): SidebarEntity[] => [
  {
    collection: "agentSummary",
    id: "same",
    value: agentSummaryFixture("same", { name }),
  },
  { collection: "agentTail", id: "same", value: { id: "same", tail } },
];
it("keeps equal native summary ids in separate generated source mirrors", () => {
  const store = createSidebarSourceStore();
  store.update(
    "local",
    sidebarSnapshot(stateFromFixture(snapshotFixture([])), facts()),
  );
  store.update(
    "remote",
    sidebarSnapshot(stateFromFixture(snapshotFixture([])), facts()),
  );
  const sources = store.sources(
    ["local", "remote"],
    new Set(["local", "remote"]),
  );
  const model = createMergedSidebarSelector()(sources);
  expect(model.rows.map((row) => row.id)).toEqual([
    model.chatKey("local", "same"),
    model.chatKey("remote", "same"),
  ]);
  expect(model.rows[0]?.agent).toBe(sources[0]?.collections.agents.get("same"));
  expect(model.rows[1]?.agent).toBe(sources[1]?.collections.agents.get("same"));
  expect(model.rows[0]?.agent.id).toBe("same");
});
it("status and tail updates leave combined routing and group membership untouched", () => {
  const store = createSidebarSourceStore();
  const snapshot = (entities: SidebarEntity[]) =>
    sidebarSnapshot(stateFromFixture(snapshotFixture([])), entities);
  store.update("local", snapshot(facts()));
  const sources = () => store.sources(["local"], new Set(["local"]));
  const select = createMergedSidebarSelector();
  const model = select(sources());
  let groups = 0;
  const stop = autorun(() => {
    groups++;
    model.rows.map((row) => row.projectPath);
  });
  try {
    const row = model.rows[0];
    const before = groups;
    store.update("local", snapshot(facts("Original", "changed tail")));
    const afterTail = select(sources());
    expect(afterTail.rows[0]).toBe(row);
    expect(groups).toBe(before);
    const changed = facts("Original", "changed tail");
    changed[0] = {
      collection: "agentSummary",
      id: "same",
      value: agentSummaryFixture("same", {
        operationalStatus: { reason: "working" },
        inFlight: true,
      }),
    };
    store.update("local", snapshot(changed));
    expect(select(sources()).rows[0]).toBe(row);
    expect(groups).toBe(before);
  } finally {
    stop();
  }
});

it("rebinds computed project keys when retained routing metadata changes", () => {
  const store = createSidebarSourceStore();
  const agent = agentSummaryFixture("same", {
    projectPath: "/local",
    projectId: "p",
    projectServerId: "local-backend",
  });
  const project = {
    id: "p",
    path: "/local",
    name: "Project",
    homeServerId: "local-backend",
  };
  const snapshot = (aliased: boolean) =>
    sidebarSnapshot(stateFromFixture(snapshotFixture([])), [
      { collection: "agentSummary", id: agent.id, value: agent },
      {
        collection: "project",
        id: project.id,
        value: {
          ...project,
          ...(aliased
            ? {
                projectAliases: [
                  {
                    serverId: "remote-backend",
                    projectId: "remote-p",
                    name: "Remote",
                  },
                ],
              }
            : {}),
        },
      },
    ]);
  store.update("local", snapshot(false));
  const select = createMergedSidebarSelector(),
    sources = () => store.sources(["local"], new Set(["local"]));
  const model = select(sources());
  const keys: string[] = [];
  const stop = autorun(() => {
    keys.push(model.rows[0]?.projectPath || "");
  });
  try {
    store.update("local", snapshot(true));
    const changed = select(sources());
    expect(keys.at(-1)).toBe(changed.projectKey("local", "/local"));
    expect(keys.at(-1)).not.toBe(keys[0]);
    expect(changed.rows[0]?.agent.id).toBe("same");
  } finally {
    stop();
  }
});

it("observes in-place generated project edits without replacing the source envelope", () => {
  const store = createSidebarSourceStore();
  const snapshot = (name: string, folder: string) =>
    sidebarSnapshot(stateFromFixture(snapshotFixture([])), [
      {
        collection: "project",
        id: "p",
        value: {
          id: "p",
          path: "/p",
          name,
          folders: [{ id: "f", name: folder, parentId: null }],
        },
      },
    ]);
  store.update("local", snapshot("Before", "Folder before"));
  const sources = store.sources(["local"], new Set(["local"]));
  const select = createMergedSidebarSelector();
  const initial = select(sources);
  const native = sources[0]!.collections.projects.get("p");
  store.update("local", snapshot("After", "Folder after"));
  expect(sources[0]!.collections.projects.get("p")).toBe(native);
  const changed = select(sources);
  expect(changed.sidebar.projects[0]?.name).toBe("After");
  expect(changed.sidebar.projects[0]?.folders?.[0]?.name).toBe("Folder after");
  expect(changed.sidebar.projects[0]?.path).toBe(
    initial.sidebar.projects[0]?.path,
  );
});

it("tracks generated membership and folder compatibility through a retained source", () => {
  const store = createSidebarSourceStore();
  const project = (path: string, name: string): SidebarEntity => ({
    collection: "project",
    id: "p",
    value: {
      id: "p",
      path,
      name,
      folders: [{ id: "f", name: "Folder", parentId: null }],
    },
  });
  store.update(
    "local",
    sidebarSnapshot(stateFromFixture(snapshotFixture([])), [
      project("/before", "Before"),
    ]),
  );
  const sources = store.sources(["local"], new Set(["local"]));
  const select = createMergedSidebarSelector();
  const names: string[][] = [];
  const stop = autorun(() =>
    names.push(select(sources).sidebar.projects.map((p) => p.name || "")),
  );
  try {
    store.update(
      "local",
      sidebarSnapshot(stateFromFixture(snapshotFixture([])), [
        project("/after", "After"),
      ]),
    );
    expect(names.at(-1)).toEqual(["After"]);
    store.update(
      "local",
      sidebarSnapshot(stateFromFixture(snapshotFixture([])), []),
    );
    expect(names.at(-1)).toEqual([]);
  } finally {
    stop();
  }
});

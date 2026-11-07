import { sidebarRowFixture } from "./sidebarRowFixture";
import { StudioErrorLevel } from "../generated/api";
import { StudioErrorEffectCertainty } from "../generated/api";
import { required } from "../components/componentTestFixtures.ts";
import assert from "node:assert/strict";
import { decodeSidebarRanks } from "./sidebarOrderRanks";
import { describe, expect, it, vi } from "vitest";
import type { Project } from "../types";
import {
  localSidebarBackend,
  type SidebarBackend,
} from "../components/sidebar/services";
import { itemGroup } from "../components/sidebar/catalog";
import {
  createMergedSidebarSelector,
  createSidebarSourceStore,
  mergeSidebar,
  type SidebarSource,
} from "./mergedSidebar";
import {
  mergedSidebarPreferences,
  sidebarOrderWrite,
} from "./mergedSidebarPreferences";
import {
  mergePreferenceFields,
  type PreferenceFields,
} from "../sync/uiPreferenceMerge";

const path = "/same/project";
const scope = "/same/state";
const treeKey = (id: string) => JSON.stringify([path, "folder", id]);
const agent = sidebarRowFixture;
function source(
  id: string,
  projects: Project[] = [{ id: path, path, name: id, folders: [] }],
): SidebarSource {
  return {
    id,
    online: true,
    sidebar: {
      stateDir: scope,
      projects,
      rows: [
        agent("same", {
          pinned: true,
          indicator: { kind: "answer", label: "Needs an answer" },
        }),
      ],
      rooms: [
        {
          id: "room",
          name: "Shared",
          projectPath: path,
          members: ["same"],
          radio: {
            controls: {
              busy: false,
              inputBlocked: false,
              canPass: false,
              stopping: false,
              speaking: false,
              busyNotice: null,
            },
            direct: true,
            active: null,
            error: null,
            next: [],
            revision: 0,
            status: "idle",
            speaker: null,
            teamId: "",
          },
        },
      ],
      peerTeams: [],
      peerTeamsVersion: 1,
      projectOrganizationVersion: 1,
      sidebarOrder: {
        revision: 3,
        groups: { projects: [path], [itemGroup(path)]: ["same", "missing"] },
      },
      savedOrder: {},
      compact: { [path]: false },
      collapsed: {},
    },
  };
}
function backend(value: SidebarSource) {
  const values = new Map<string, string>();
  // This window's preference fields with edit times, as uiPreferenceStore keeps them.
  const fields: PreferenceFields = {};
  const writes: string[] = [];
  const listeners = new Map<string, Set<() => void>>();
  const storage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, data: string) => {
      values.set(key, data);
      writes.push(key);
    },
    removeItem: (key: string) => {
      values.delete(key);
      writes.push(key);
    },
  };
  const result: SidebarBackend = {
    ...localSidebarBackend,
    ownerId: value.id,
    storage,
    saved<T>(key: string, fallback: T): T {
      return values.has(key) ? JSON.parse(values.get(key)!) : fallback;
    },
    save(key, data) {
      storage.setItem(key, JSON.stringify(data));
    },
    editPreference(key, previous, next) {
      storage.setItem(key, JSON.stringify(next));
      const timestamp = Math.max(
        Date.now(),
        ...Object.values(fields).map((field) => field.timestamp + 1),
      );
      for (const name of new Set([
        ...Object.keys(previous),
        ...Object.keys(next),
      ]))
        if (previous[name] !== next[name])
          fields[JSON.stringify([key, name])] = {
            value: (next[name] as boolean | undefined) ?? null,
            timestamp,
            writer: "this-window",
          };
      listeners.get(key)?.forEach((listener) => listener());
      return next;
    },
    preferenceFields: () => ({ ...fields }),
    adoptPreferenceFields(incoming) {
      Object.assign(fields, mergePreferenceFields(fields, incoming));
    },
    subscribePreference(key, update) {
      const callbacks = listeners.get(key) || new Set();
      callbacks.add(update);
      listeners.set(key, callbacks);
      return () => {
        callbacks.delete(update);
      };
    },
  };
  return { backend: result, values, writes, listeners, fields };
}

describe("complete multi-server sidebar projection", () => {
  it("groups each server's line-backed chat under its qualified project", () => {
    const local = source("local");
    const remote = source("remote");
    for (const value of [local, remote]) {
      value.sidebar.rows = [
        agent("lead", {
          projectPath: path,
          projectFolder: "notes",
        }),
      ];
      value.sidebar.projects[0]!.folders = [{ id: "notes", name: "Notes" }];
    }
    const model = mergeSidebar([local, remote]);
    for (const owner of ["local", "remote"]) {
      const key = model.projectKey(owner, path);
      const chat = model.rows.find((row) => row.projectPath === key);
      expect(chat?.id).toBe(model.chatKey(owner, "lead"));
      expect(model.chatReferences.get(chat!.id)?.path).toBe(path);
      expect(model.folderReferences.get(chat!.projectFolder!)?.owner).toBe(
        owner,
      );
    }
    expect(new Set(model.rows.map((row) => row.projectPath)).size).toBe(2);
  });
  it("keeps render identities stable while a paired source is still loading", () => {
    const local = source("local");
    const remote = source("remote");
    const select = createMergedSidebarSelector();
    const first = select([local]);
    const complete = select([local, remote]);
    expect(first.data.stateDir).toBe(complete.data.stateDir);
    assert.ok(first.rows[0] != null);
    assert.ok(complete.rows[0] != null);
    expect(first.rows[0].id).toBe(complete.rows[0].id);
    assert.ok(first.data.runtime.projects[0] != null);
    assert.ok(complete.data.runtime.projects[0] != null);
    expect(first.data.runtime.projects[0].path).toBe(
      complete.data.runtime.projects[0].path,
    );
  });

  it("keeps an unavailable home explicit instead of routing metadata to an alias server", () => {
    const remote = source("remote", [
      {
        id: path,
        path,
        name: "Remote folder",
        projectAliases: [
          {
            serverId: "unpaired-home",
            projectId: "project:home",
            name: "Home",
          },
        ],
      },
    ]);
    const model = mergeSidebar([remote], 1, true);
    const project = model.data.runtime.projects[0];
    assert.ok(project != null);
    expect(model.projectReferences.get(project.path!)?.owner).toBe(
      "unpaired-home",
    );
    expect(model.sourceById.has("unpaired-home")).toBe(false);
    expect(model.wireProject("remote", project.path!)).toBe(path);
  });

  it("preserves single-server identities, objects, storage and null order semantics", () => {
    const local = source("local");
    local.sidebar.sidebarOrder = { revision: 0, groups: null };
    local.sidebar.savedOrder = { projects: [path] };
    local.sidebar.studioErrors = [
      {
        id: "event:local",
        kind: "input_uncertain",
        level: StudioErrorLevel.error,
        diagnostics: {
          context: { source: "input", turnId: null, epoch: 0 },
          certainty: StudioErrorEffectCertainty.unknown,
          causes: [],
        },
        action: "resume",
        agentId: "same",
        agentName: "Local",
        reason: "Delivery uncertain",
        count: 1,
        firstAt: 100,
        at: 100,
        detail: "",
        subject: "",
      },
    ];
    const model = mergeSidebar([local]);
    const original = backend(local);
    expect(model.rows).toEqual(local.sidebar.rows);
    expect(model.rows[0]).toBe(local.sidebar.rows[0]);
    expect(model.data.runtime.projects[0]).toBe(local.sidebar.projects[0]);
    expect(model.data.runtime.rooms[0]).toBe(local.sidebar.rooms[0]);
    expect(model.data.runtime.sidebarOrder).toBe(local.sidebar.sidebarOrder);
    expect(model.data.runtime.studioErrors).toBe(local.sidebar.studioErrors);
    expect(model.data.stateDir).toBe(scope);
    expect(
      mergedSidebarPreferences(model, new Map([["local", original.backend]])),
    ).toBe(original.backend);
    expect(original.writes).toEqual([]);
  });

  it("creates inferred project groups only from server-provided rows", () => {
    const local = source("local");
    local.sidebar.rows = [agent("visible", { projectPath: "/elsewhere/lead" })];
    const model = mergeSidebar([local, source("remote")]);
    expect(
      model.data.runtime.projects.map((project) => project.path),
    ).toContain(model.projectKey("local", "/elsewhere/lead"));
    expect(model.data.threads).toEqual([]);
    expect(model.data.runtime.agents).toEqual([]);
  });

  it("separates overlapping chat, room, folder and team IDs", () => {
    const a = source("local");
    const b = source("remote");
    for (const value of [a, b]) {
      assert.ok(value.sidebar.projects[0] != null);
      value.sidebar.projects[0].folders = [{ id: "folder", name: "Notes" }];
      assert.ok(value.sidebar.rows[0] != null);
      value.sidebar.rows = [
        { ...value.sidebar.rows[0], projectFolder: "folder" },
      ];
      value.sidebar.peerTeams = [
        { id: "team", name: value.id, projectPath: path, members: ["same"] },
      ];
    }
    const model = mergeSidebar([a, b]);
    expect(model.data.runtime.projects).toHaveLength(2);
    expect(new Set(model.rows.map((row) => row.id)).size).toBe(2);
    expect(new Set(model.data.runtime.rooms.map((row) => row.id)).size).toBe(2);
    expect(
      new Set(model.data.runtime.peerTeams.map((row) => row.id)).size,
    ).toBe(2);
    for (const value of [a, b]) {
      const lead = model.rows.find(
        (row) => row.id === model.chatKey(value.id, "same"),
      )!;
      expect(model.folderReferences.get(lead.projectFolder!)).toEqual({
        owner: value.id,
        id: "folder",
        path,
      });
      expect(model.chatReferences.get(lead.id)).toEqual({
        owner: value.id,
        id: "same",
        path,
      });
      expect(lead.indicator).toBe(value.sidebar.rows[0]?.indicator);
    }
    expect(() => mergeSidebar([a, a])).toThrow("Duplicate sidebar source");
  });

  it("uses one home heading and keeps conflicting remote branches, independent of arrival order", () => {
    const home = source("local", [
      {
        id: "project:shared",
        path: "project:shared",
        name: "Home project",
        homeServerId: "home-id",
        locations: [
          { serverId: "home-id", path, projectId: path },
          {
            serverId: "remote",
            path: "/remote/path",
            projectId: "/remote/path",
          },
        ],
        folders: [
          { id: "same", name: "Home folder" },
          { id: "nested", name: "Child", parentId: "same" },
          { id: "compatible", name: "Shared folder" },
        ],
      },
    ]);
    const remote = source("remote", [
      {
        id: "/remote/path",
        path: "/remote/path",
        name: "Remote project",
        homeServerId: "remote",
        projectAliases: [
          {
            serverId: "home-id",
            projectId: "project:shared",
            name: "Home project",
          },
        ],
        folders: [
          { id: "same", name: "Remote folder" },
          { id: "nested", name: "Child", parentId: "same" },
          { id: "compatible", name: "Shared folder" },
        ],
      },
    ]);
    remote.sidebar.rows = [
      agent("same", {
        projectPath: "/remote/path",
        projectId: "project:shared",
        projectServerId: "home-id",
        projectFolder: "nested",
      }),
    ];
    remote.sidebar.peerTeams = [
      {
        id: "team",
        name: "Remote team",
        projectPath: "/remote/path",
        members: ["same"],
      },
    ];
    const model = mergeSidebar([remote, home]);
    expect(model.data.runtime.projects).toHaveLength(1);
    const project = model.data.runtime.projects[0];
    assert.ok(project != null);
    expect(project.name).toBe("Home project");
    expect(project.folders?.map((row) => row.name)).toEqual([
      "Home folder",
      "Child",
      "Shared folder",
      "Remote folder",
      "Child",
    ]);
    assert.ok(model.rows[0] != null);
    const folder = model.folderReferences.get(model.rows[0].projectFolder!);
    expect(folder?.owner).toBe("remote");
    const nested = project.folders!.find(
      (row) => row.id === required(model.rows[0]).projectFolder,
    );
    expect(model.folderReferences.get(nested!.parentId!)?.owner).toBe("remote");
    expect(model.rows.every((row) => row.projectPath === project.path)).toBe(
      true,
    );
    expect(required(model.data.runtime.peerTeams[0]).projectPath).toBe(
      project.path,
    );
    expect(model.wireProject("remote", project.path!)).toBe("/remote/path");
    expect(model.wireProject("local", project.path!)).toBe("project:shared");
  });

  it("retains server ordering, archive, search text, indicator and read CAS proof", () => {
    const local = source("local");
    local.sidebar.rows = [
      agent("z", {
        sortKey: "a",
        archived: true,
        searchText: "server search",
        markRead: {
          threadId: "native",
          turnId: "turn",
          revision: 9,
          read: true,
        },
      }),
      agent("a", { sortKey: "b" }),
    ];
    const model = mergeSidebar([local, source("remote")]);
    expect(model.rows.slice(0, 2).map((row) => row.id)).toEqual([
      model.chatKey("local", "z"),
      model.chatKey("local", "a"),
    ]);
    expect(model.rows[0]?.markRead).toBe(local.sidebar.rows[0]?.markRead);
    expect(model.rows[0]?.searchText).toBe("server search");
    expect(model.rows[0]?.archived).toBe(true);
  });

  it("retains offline rows on summary updates and shares unchanged projected rows", () => {
    const local = source("local");
    const remote = source("remote");
    const store = createSidebarSourceStore();
    store.update("local", local.sidebar);
    store.update("remote", remote.sidebar);
    store.update("remote", undefined);
    const sources = store.sources(["local", "remote"], new Set(["local"]));
    assert.ok(sources[1] != null);
    expect(sources[1].sidebar).toBe(remote.sidebar);
    expect(sources[1].online).toBe(false);
    const select = createMergedSidebarSelector();
    const before = select(sources);
    expect(select(sources)).toBe(before);
    assert.ok(sources[0]);
    assert.ok(local.sidebar.projects[0]);
    const next = select([
      {
        ...sources[0],
        sidebar: {
          ...local.sidebar,
          projects: [{ ...local.sidebar.projects[0], name: "Renamed" }],
        },
      },
      sources[1],
    ]);
    expect(next.rows[0]).toBe(before.rows[0]);
    // A snapshot for another reason keeps the order revision, so a waiting move stays.
    expect(next.data.runtime.sidebarOrder).toBe(
      before.data.runtime.sidebarOrder,
    );
    const moved = select([
      {
        ...sources[0],
        sidebar: {
          ...local.sidebar,
          sidebarOrder: {
            revision: 4,
            groups: {
              ...local.sidebar.sidebarOrder!.groups,
              [itemGroup(path)]: ["missing", "same"],
            },
          },
        },
      },
      sources[1],
    ]);
    expect(moved.data.runtime.sidebarOrder!.revision).toBeGreaterThan(
      before.data.runtime.sidebarOrder!.revision,
    );
    store.forget("remote");
    expect(
      store.sources(["local", "remote"], new Set()).map((row) => row.id),
    ).toEqual(["local"]);
  });

  it("maps unreachable sources to server identities and clears on reconnect", () => {
    const local = source("local");
    const remote = { ...source("remote"), online: false };
    const offline = mergeSidebar([local, remote], 1, true);
    expect([...offline.offlineServers]).toEqual(["remote"]);
    expect(offline.offlineServers.has("local")).toBe(false);
    expect(
      mergeSidebar([local, { ...remote, online: true }], 2, true)
        .offlineServers,
    ).toEqual(new Set());
  });
});

describe("original saved sidebar state", () => {
  it.each([undefined, false, true])(
    "ignores saved project collapse state in every source (remote: %s)",
    (remoteValue) => {
      const local = source("local");
      const remote = source("remote", [
        {
          id: path,
          path,
          name: "Remote",
          projectAliases: [
            { serverId: "local", projectId: path, name: "local" },
          ],
        },
      ]);
      local.sidebar.collapsed = { [path]: true };
      remote.sidebar.collapsed =
        remoteValue === undefined ? {} : { [path]: remoteValue };
      const model = mergeSidebar([local, remote]);
      const heading = model.projectKey("local", path);
      expect(model.projectKey("remote", path)).toBe(heading);
      expect(model.collapsed[heading]).toBeUndefined();
      local.sidebar.collapsed = {};
      remote.sidebar.collapsed = { [path]: true };
      expect(mergeSidebar([local, remote]).collapsed[heading]).toBeUndefined();
    },
  );

  it("keeps a compatible remote folder open when its home folder is collapsed", () => {
    const local = source("local");
    const remote = source("remote", [
      {
        id: path,
        path,
        name: "Remote",
        projectAliases: [{ serverId: "local", projectId: path, name: "local" }],
      },
    ]);
    for (const value of [local, remote])
      required(value.sidebar.projects[0]).folders = [
        { id: "shared", name: "Notes" },
      ];
    local.sidebar.collapsed = { [treeKey("shared")]: true };
    const model = mergeSidebar([local, remote]);
    expect(model.mapTreeKey("local", treeKey("shared"))).toBe(
      model.mapTreeKey("remote", treeKey("shared")),
    );
    expect(model.collapsed[model.mapTreeKey("local", treeKey("shared"))]).toBe(
      false,
    );
    expect(local.sidebar.collapsed).toEqual({ [treeKey("shared")]: true });
    expect(remote.sidebar.collapsed).toEqual({});
  });

  it("uses the owner's saved compact preference despite its stale snapshot", () => {
    const local = source("local");
    const remote = source("remote");
    const a = backend(local);
    const b = backend(remote);
    a.values.set(
      `codex-project-tree:${scope}`,
      JSON.stringify({ [path]: true }),
    );
    b.values.set(
      `codex-project-compact:${scope}`,
      JSON.stringify({ [path]: true }),
    );
    const model = mergeSidebar([local, remote]);
    const cache = mergedSidebarPreferences(
      model,
      new Map([
        ["local", a.backend],
        ["remote", b.backend],
      ]),
    );
    expect(
      cache.saved<Record<string, boolean>>(
        `codex-project-tree:${model.data.stateDir}`,
        {},
      )[model.projectKey("local", path)],
    ).toBeUndefined();
    expect(
      cache.saved<Record<string, boolean>>(
        `codex-project-compact:${model.data.stateDir}`,
        {},
      )[model.projectKey("remote", path)],
    ).toBe(true);
    expect(a.writes).toEqual([]);
    expect(b.writes).toEqual([]);
    const compactKey = `codex-project-compact:${model.data.stateDir}`;
    const field = model.projectKey("remote", path);
    cache.editPreference(compactKey, { [field]: true }, { [field]: false });
    expect(b.values.get(`codex-project-compact:${scope}`)).toBe(
      JSON.stringify({ [path]: false }),
    );
    const reopened = mergedSidebarPreferences(
      model,
      new Map([
        ["local", a.backend],
        ["remote", b.backend],
      ]),
    );
    expect(reopened.saved<Record<string, boolean>>(compactKey, {})[field]).toBe(
      false,
    );
  });

  it("reads original namespaces without writing and ignores project collapse with shared compact visibility", () => {
    const local = source("local");
    const remote = source("remote", [
      {
        id: path,
        path,
        name: "Remote",
        projectAliases: [{ serverId: "local", projectId: path, name: "local" }],
      },
    ]);
    local.sidebar.compact = { [path]: true };
    local.sidebar.collapsed = { [path]: true };
    remote.sidebar.compact = { [path]: false };
    const a = backend(local);
    const b = backend(remote);
    const model = mergeSidebar([local, remote]);
    const cache = mergedSidebarPreferences(
      model,
      new Map([
        ["local", a.backend],
        ["remote", b.backend],
      ]),
    );
    expect(
      cache.saved(`codex-project-compact:${model.data.stateDir}`, {}),
    ).toEqual({ [model.projectKey("local", path)]: false });
    // The saved bare path of the earlier project collapse stays unused.
    expect(
      cache.saved(`codex-project-tree:${model.data.stateDir}`, {}),
    ).toEqual({
      [JSON.stringify([model.projectKey("local", path), "project"])]: false,
    });
    expect(a.writes).toEqual([]);
    expect(b.writes).toEqual([]);
    expect(local.sidebar.collapsed).toEqual({ [path]: true });
  });

  it("shows each collapse by its latest edit time, however late a snapshot arrives", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000_000);
    const local = source("local");
    const remote = source("remote");
    const a = backend(local);
    const b = backend(remote);
    const backends = new Map([
      ["local", a.backend],
      ["remote", b.backend],
    ]);
    const heading = JSON.stringify([path, "project"]);
    const field = JSON.stringify([`codex-project-tree:${scope}`, heading]);
    const tree = (model: ReturnType<typeof mergeSidebar>) =>
      `codex-project-tree:${model.data.stateDir}`;
    const shown = (model: ReturnType<typeof mergeSidebar>, key: string) =>
      mergedSidebarPreferences(model, backends).saved<Record<string, boolean>>(
        tree(model),
        {},
      )[key];
    const snapshot = (value: boolean, timestamp: number) => {
      const row = source("local");
      row.sidebar.collapsed = { [heading]: value };
      row.sidebar.visualFields = {
        [field]: { value, timestamp, writer: "frame" },
      };
      return mergeSidebar([row, source("remote")]);
    };
    const first = mergeSidebar([local, remote]);
    const key = JSON.stringify([first.projectKey("local", path), "project"]);
    const cache = mergedSidebarPreferences(first, backends);
    const previous = cache.saved<Record<string, boolean>>(tree(first), {});
    cache.editPreference(tree(first), previous, { ...previous, [key]: true });
    // A snapshot that does not carry the edit yet does not hide it.
    expect(shown(mergeSidebar([source("local"), source("remote")]), key)).toBe(
      true,
    );
    // A late echo of an earlier value does not flip it back, at any time.
    expect(shown(snapshot(false, 999_000), key)).toBe(true);
    vi.setSystemTime(1_000_000 + 60_000);
    expect(shown(snapshot(false, 999_000), key)).toBe(true);
    // A newer change from another window wins.
    const newer = snapshot(false, 1_000_000 + 50_000);
    expect(shown(newer, key)).toBe(false);
    // The next edit in this window is newer than the value it replaces.
    const view = mergedSidebarPreferences(newer, backends);
    const before = view.saved<Record<string, boolean>>(tree(newer), {});
    view.editPreference(tree(newer), before, { ...before, [key]: true });
    expect(shown(snapshot(false, 1_000_000 + 50_000), key)).toBe(true);
    expect(a.fields[field]).toMatchObject({ value: true });
    assert.ok(a.fields[field] != null);
    expect(a.fields[field].timestamp).toBeGreaterThan(1_000_000 + 50_000);
    vi.useRealTimers();
  });

  it("collapses a project heading by its home server's value and saves it there", () => {
    const local = source("local");
    const remote = source("remote", [
      {
        id: path,
        path,
        name: "Remote",
        projectAliases: [{ serverId: "local", projectId: path, name: "local" }],
      },
    ]);
    const heading = JSON.stringify([path, "project"]);
    remote.sidebar.collapsed = { [heading]: true };
    const a = backend(local);
    const b = backend(remote);
    const model = mergeSidebar([local, remote]);
    const key = JSON.stringify([model.projectKey("local", path), "project"]);
    expect(model.mapTreeKey("remote", heading)).toBe(key);
    // A member server's value does not collapse the home project.
    expect(model.collapsed[key]).toBe(false);
    const cache = mergedSidebarPreferences(
      model,
      new Map([
        ["local", a.backend],
        ["remote", b.backend],
      ]),
    );
    const tree = `codex-project-tree:${model.data.stateDir}`;
    const previous = cache.saved<Record<string, boolean>>(tree, {});
    expect(
      cache.editPreference(tree, previous, { ...previous, [key]: true }),
    ).toMatchObject({ [key]: true });
    expect(JSON.parse(a.values.get(`codex-project-tree:${scope}`)!)).toEqual({
      [heading]: true,
    });
    expect(b.writes).toEqual([]);
    local.sidebar.collapsed = { [heading]: true };
    remote.sidebar.collapsed = {};
    expect(mergeSidebar([local, remote]).collapsed[key]).toBe(true);
  });

  it("writes only changed source fields, keeps unrelated keys and subscribes to original keys", () => {
    const local = source("local");
    const remote = source("remote");
    assert.ok(remote.sidebar.projects[0] != null);
    remote.sidebar.projects[0].folders = [
      { id: "folder", name: "Remote folder" },
    ];
    const a = backend(local);
    const b = backend(remote);
    remote.sidebar.collapsed = { [treeKey("folder")]: true, "/other": true };
    b.values.set(
      `codex-project-tree:${scope}`,
      JSON.stringify({ [treeKey("folder")]: true, "/other": true }),
    );
    const model = mergeSidebar([local, remote]);
    const cache = mergedSidebarPreferences(
      model,
      new Map([
        ["local", a.backend],
        ["remote", b.backend],
      ]),
    );
    const key = `codex-project-tree:${model.data.stateDir}`;
    const previous = cache.saved<Record<string, boolean>>(key, {});
    const mapped = model.mapTreeKey("remote", treeKey("folder"));
    let updates = 0;
    const stop = cache.subscribePreference(key, () => updates++);
    cache.editPreference(key, previous, { ...previous, [mapped]: false });
    expect(a.writes).toEqual([]);
    expect(b.writes).toEqual([`codex-project-tree:${scope}`]);
    expect(b.backend.saved(`codex-project-tree:${scope}`, {})).toEqual({
      [treeKey("folder")]: false,
      "/other": true,
    });
    expect(updates).toBe(1);
    stop();
    expect(
      [...b.listeners.values()].every((listeners) => !listeners.size),
    ).toBe(true);
  });

  it("refuses offline preference changes before touching any source", () => {
    const local = source("local");
    const remote = source("remote", [
      {
        id: path,
        path,
        name: "Remote",
        projectAliases: [{ serverId: "local", projectId: path, name: "local" }],
      },
    ]);
    local.online = false;
    const a = backend(local);
    const b = backend(remote);
    const model = mergeSidebar([local, remote]);
    const cache = mergedSidebarPreferences(
      model,
      new Map([
        ["local", a.backend],
        ["remote", b.backend],
      ]),
    );
    const key = `codex-project-compact:${model.data.stateDir}`;
    const previous = cache.saved<Record<string, boolean>>(key, {});
    expect(() =>
      cache.editPreference(key, previous, {
        ...previous,
        [model.projectKey("local", path)]: true,
      }),
    ).toThrow("offline");
    expect(a.writes).toEqual([]);
    expect(b.writes).toEqual([]);
  });

  it("saves collapse choices for teams supplied only by project metadata", () => {
    const local = source("local");
    const remote = source("remote");
    assert.ok(remote.sidebar.projects[0] != null);
    remote.sidebar.projects[0].peerTeams = [
      { id: "legacy-team", name: "Team", members: ["same"] },
    ];
    const a = backend(local);
    const b = backend(remote);
    const model = mergeSidebar([local, remote]);
    const cache = mergedSidebarPreferences(
      model,
      new Map([
        ["local", a.backend],
        ["remote", b.backend],
      ]),
    );
    const key = `codex-project-tree:${model.data.stateDir}`;
    const native = JSON.stringify([path, "team", "legacy-team"]);
    const previous = cache.saved<Record<string, boolean>>(key, {});
    cache.editPreference(key, previous, {
      ...previous,
      [model.mapTreeKey("remote", native)]: true,
    });
    expect(
      b.backend.saved<Record<string, boolean>>(
        `codex-project-tree:${scope}`,
        {},
      )[native],
    ).toBe(true);
    expect(a.writes).toEqual([]);
    expect(b.writes).toEqual([`codex-project-tree:${scope}`]);
  });

  it("saves an explicit mixed reorder on its owner without changing original or unknown IDs", () => {
    const local = source("local");
    const remote = source("remote");
    const original = JSON.stringify([local.sidebar, remote.sidebar]);
    const model = mergeSidebar([local, remote]);
    const group = itemGroup(model.projectKey("local", path));
    const ids = [...required(model.order[group])].reverse();
    const write = sidebarOrderWrite(model, "local", {
      ...model.order,
      [group]: ids,
    });
    expect(write.expected_revision).toBe(3);
    expect(write.groups[itemGroup(path)]).toEqual(["missing", "same"]);
    expect(
      decodeSidebarRanks(
        required(
          write.groups[JSON.stringify(["combined-sidebar-ranks", group])],
        ),
      ),
    ).toEqual(ids);
    expect(write.groups.projects).toEqual([path]);
    expect(JSON.stringify([local.sidebar, remote.sidebar])).toBe(original);
    const accepted = {
      ...local,
      sidebar: {
        ...local.sidebar,
        sidebarOrder: { revision: 4, groups: write.groups },
      },
    };
    expect(mergeSidebar([accepted, remote]).order[group]).toEqual(ids);
    expect(() =>
      sidebarOrderWrite(
        mergeSidebar([local, { ...remote, online: false }]),
        "remote",
        model.order,
      ),
    ).toThrow("offline");
  });
});

it("preserves source order when server sort keys overlap", () => {
  const local = source("local"),
    remote = source("remote");
  local.sidebar.rows = [agent("a", { sortKey: "same" })];
  remote.sidebar.rows = [agent("a", { sortKey: "same" })];
  const select = createMergedSidebarSelector();
  const model = select([local, remote]);
  expect(model.rows.map((row) => row.id)).toEqual([
    model.chatKey("local", "a"),
    model.chatKey("remote", "a"),
  ]);
  expect(model.rows[0]?.sortKey).toBe("0000000000\u0000same");
  expect(model.rows[1]?.sortKey).toBe("0000000001\u0000same");
});

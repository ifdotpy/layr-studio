import { StudioErrorLevel } from "../generated/api";
import { StudioErrorEffectCertainty } from "../generated/api";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Snapshot } from "../types";
import { preferenceMemory } from "./storage";
import { navigationSnapshot } from "./navigation";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";
import { agentEntities, sidebarFactsFixture } from "./sidebarSourceFixture.ts";
import { sidebarSnapshot } from "./sidebarSnapshot";

function snapshot(): Snapshot {
  return {
    token: "session-token",
    stateDir: "/state/original",
    threads: [],
    chats: [],
    nodes: [],
    edges: [],
    runtime: {
      agents: [],
      rooms: [],
      tasks: [],
      monitors: [],
      complaints: [],
      requests: [],
      rules: [],
      projects: [],
      peerTeams: [],
      events: [],
      work: [],
      studioErrors: [],
    },
  };
}

let records: Map<string, string>;
let getItem: ReturnType<typeof vi.fn>;
let setItem: ReturnType<typeof vi.fn>;
beforeEach(() => {
  preferenceMemory.clear();
  records = new Map();
  getItem = vi.fn((key: string) => records.get(key) ?? null);
  setItem = vi.fn();
  vi.stubGlobal("localStorage", { getItem, setItem });
});
afterEach(() => vi.unstubAllGlobals());

describe("server sidebar projection", () => {
  it("forwards native summary facts unchanged without stage1 presentation rows", () => {
    const data = snapshot();
    data.threads = [{ id: "lead", name: "Stale Agent title", inFlight: true }];
    const entities = agentEntities([
      agentSummaryFixture("lead", {
        markRead: {
          threadId: "native",
          turnId: "turn",
          read: true,
          revision: 5,
        },
      }),
    ]);
    const projected = sidebarSnapshot(data, entities);
    expect(projected.entities).toBe(entities);
    expect(projected).not.toHaveProperty("rows");
    expect(projected).not.toHaveProperty("threads");
    expect(projected).not.toHaveProperty("indicators");
    expect(projected).not.toHaveProperty("unread");
    expect(JSON.stringify(projected)).not.toContain("session-token");
    expect(JSON.stringify(projected)).not.toContain("Stale Agent title");
  });

  it("reads the original saved keys and preserves every group, folder and team key", () => {
    const data = snapshot();
    const folderKey = JSON.stringify(["/work", "folder", "nested"]);
    const teamKey = JSON.stringify(["/work", "team", "team"]);
    const itemGroup = JSON.stringify(["items", "/work", "nested"]);
    const legacyGroup = JSON.stringify([
      "chats",
      "/work",
      true,
      false,
      "team",
      "team",
    ]);
    const compact = { "/work": false, "/other": true };
    const collapsed = { "/work": true, [folderKey]: false, [teamKey]: true };
    const groups = {
      projects: ["/work"],
      [itemGroup]: ["lead", "folder:nested"],
      [legacyGroup]: ["second"],
    };
    preferenceMemory.setItem(
      `codex-project-compact:${data.stateDir}`,
      JSON.stringify(compact),
    );
    preferenceMemory.setItem(
      `codex-project-tree:${data.stateDir}`,
      JSON.stringify(collapsed),
    );
    records.set(`codex-sidebar-order:${data.stateDir}`, JSON.stringify(groups));
    preferenceMemory.setItem(
      "codex-project-compact:/state/different",
      JSON.stringify({ "/work": true }),
    );
    // Edit times of this server's sidebar fields go with the snapshot; other keys do not.
    const field = (key: string, name: string) => JSON.stringify([key, name]);
    const own = field(`codex-project-tree:${data.stateDir}`, folderKey);
    preferenceMemory.setItem(
      "studio-ui-preference-fields-v1:server",
      JSON.stringify({
        [own]: { value: false, timestamp: 7, writer: "frame" },
        [field("codex-project-tree:/state/different", folderKey)]: {
          value: true,
          timestamp: 8,
          writer: "frame",
        },
        [field("codex-worker-disclosures", "lead")]: {
          value: true,
          timestamp: 9,
          writer: "frame",
        },
      }),
    );
    const projected = sidebarSnapshot(data);
    expect(projected.visualFields).toEqual({
      [own]: { value: false, timestamp: 7, writer: "frame" },
    });
    expect(projected.compact).toEqual(compact);
    expect(projected.collapsed).toEqual(collapsed);
    expect(projected.savedOrder).toEqual(groups);
    expect(projected.sidebarOrder).toBeUndefined();
    // Preference records come from streamed memory; only the saved order reads storage.
    expect(getItem.mock.calls.map(([key]) => key).sort()).toEqual(
      [`codex-sidebar-order:${data.stateDir}`].sort(),
    );
    expect(setItem).not.toHaveBeenCalled();
  });

  it.each<Snapshot["runtime"]["sidebarOrder"]>([
    undefined,
    null,
    { revision: 4, groups: null },
    { revision: 8, groups: {} },
    { revision: 9, groups: { projects: ["/server"] } },
  ])("retains server order support and migration state: %j", (order) => {
    const data = snapshot();
    data.runtime.sidebarOrder = order;
    records.set(
      `codex-sidebar-order:${data.stateDir}`,
      JSON.stringify({ projects: ["/legacy"] }),
    );
    const projected = sidebarSnapshot(data);
    expect(projected.sidebarOrder).toEqual(order);
    expect(projected.savedOrder).toEqual({ projects: ["/legacy"] });
  });

  it("uses empty saved preferences when keys are missing or invalid", () => {
    const data = snapshot();
    records.set(`codex-project-compact:${data.stateDir}`, "invalid");
    records.set(`codex-project-tree:${data.stateDir}`, "null");
    expect(sidebarSnapshot(data)).toMatchObject({
      compact: {},
      collapsed: {},
      savedOrder: {},
    });
  });

  it("forwards streamed Studio errors without changing their action ids", () => {
    const data = snapshot();
    const item: Snapshot["runtime"]["studioErrors"][number] = {
      id: "agent:worker:turn",
      kind: "turn_disconnected",
      level: StudioErrorLevel.error,
      diagnostics: {
        context: { source: "agent", turnId: "turn", epoch: 0 },
        certainty: StudioErrorEffectCertainty.unknown,
        causes: [],
      },
      action: "resume",
      agentId: "worker",
      agentName: "Worker",
      reason: "Connection ended",
      count: 1,
      firstAt: 100,
      at: 100,
      detail: "",
      subject: "",
    };
    data.runtime.studioErrors = [item];
    expect(sidebarSnapshot(data).studioErrors).toBe(data.runtime.studioErrors);
  });

  it("publishes navigation from raw summary facts after state is available", () => {
    const data = snapshot(),
      source = sidebarFactsFixture([
        agentSummaryFixture("lead", { inFlight: true }),
      ]);
    const navigation = navigationSnapshot(
      data,
      "lead",
      "",
      source.view.rows,
      [],
      undefined,
      new Set(),
      source.sidebar.entities,
    );
    expect(navigation.sidebar).toEqual(
      sidebarSnapshot(data, source.sidebar.entities),
    );
    expect(navigation.busy).toBe(true);
    expect(navigation.chats[0]).toMatchObject({
      kind: "chat",
      agent: { id: "lead", inFlight: true },
    });
    expect(navigationSnapshot(null, null, "")).not.toHaveProperty("sidebar");
  });
});

import type { ComponentSnapshot as Snapshot } from "../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import {
  createStateMirror,
  createRequestsMirror,
  createProjectionMetadataMirror,
  serializeStateCollections,
} from "../generated/observableResources.ts";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";
import { AgentSource } from "../generated/api.ts";
import { required } from "../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import type { ServerNavigation, ServerAccount } from "./navigation.ts";
import { createScreenChannel, type ScreenChannel } from "./screenChannel.ts";

const hooks = vi.hoisted(() => ({
  mirror: null as ReturnType<typeof createStateMirror> | null,
  screen: null as ScreenChannel | null,
}));
vi.mock("./environment.ts", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./environment.ts")>()),
  serverViewId: "remote",
  serverParentOrigin: "https://parent.example",
  serverStorageName: (name: string) => `${name}:server:remote`,
}));
vi.mock("./net/transport.ts", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./net/transport.ts")>()),
  apiOrigin: () => "https://remote.example",
}));
const stopConnection = vi.hoisted(() => vi.fn());
vi.mock("../sync/subscriptions/events.ts", () => ({
  watchResourceConnection: () => stopConnection,
  watchResourceChanges: vi.fn(),
  replaceStream: vi.fn(),
  defaultSubscriptions: { watchResourceConnection: () => stopConnection },
}));

import * as api from "./net/api.ts";
import { startServerFrame } from "./frameBridge.ts";
import { preferenceEvent } from "../sync/uiPreferenceMerge.ts";
import { writePreferenceEdit } from "../sync/uiPreferenceStore.ts";
import { preferenceMemory } from "./storage.ts";

class TestStorageEvent extends Event {
  readonly key: string | null;
  constructor(type: string, init: { key?: string | null } = {}) {
    super(type);
    this.key = init.key ?? null;
  }
}

let surface: EventTarget & {
  parent: { postMessage: ReturnType<typeof vi.fn> };
};
let records: Map<string, string>;
let cleanup: (() => void)[];
beforeEach(() => {
  preferenceMemory.clear();
  hooks.mirror = createStateMirror();
  hooks.mirror.apply([
    {
      collection: "agentSummary",
      id: "lead",
      value: agentSummaryFixture("lead", {
        name: "Real chat",
        operationalStatus: { reason: "unread_result" },
      }),
    },
  ]);
  hooks.screen = null;
  cleanup = [];
  records = new Map();
  surface = Object.assign(new EventTarget(), {
    parent: { postMessage: vi.fn() },
  });
  vi.stubGlobal("window", surface);
  vi.stubGlobal("StorageEvent", TestStorageEvent);
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => records.get(key) ?? null,
    setItem: (key: string, value: string) => records.set(key, value),
  });
  stopConnection.mockClear();
});
afterEach(() => {
  cleanup.forEach((stop) => stop());
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function mount(
  data: Snapshot | null = {
    token: "session-token",
    stateDir: "/same/state",
    threads: [
      {
        id: "lead",
        name: "Real chat",
        source: AgentSource.managed,
        isLead: true,
        cwd: "/work",
      },
    ],
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
  },
  accounts: ServerAccount[] = [],
) {
  const run = vi.fn();
  if (data)
    required(hooks.mirror).apply([
      ...serializeStateCollections(required(hooks.mirror).current).filter(
        (entity) => entity.collection !== "workspace",
      ),
      {
        collection: "workspace",
        id: "current",
        value: { stateDir: data.stateDir },
      },
    ]);
  const requests = createRequestsMirror();
  const metadata = createProjectionMetadataMirror({
    workspaceId: data?.entityWorkspaceId ?? "",
    ready: !!data,
    complete: !!data && data.entitiesComplete !== false,
    projectSequences: {},
  });
  cleanup.push(
    startServerFrame(
      {
        store: {
          requests: () => ({
            current: { status: "live", data: requests.current },
          }),
          agentDetail: () => ({ current: { status: "loading" } }),
          state: () => ({
            current: { status: "live", data: required(hooks.mirror).current },
          }),
        },
        services: {
          api: {
            get: api.get,
            saved: api.saved,
            getToken: () => data?.token ?? "",
          },
          sync: { projectionMetadata: metadata.current },
          resources: { watchResourceConnection: () => stopConnection },
        },
        screen: { channel: hooks.screen, active: true },
      },
      {
        ready: !!data,
        opened: "lead",
        error: "",
        run,
        pendingReads: new Map(),
        accounts,
        creating: false,
      },
    ),
  );
  return run;
}

it.each(["parent", "screen"])(
  "preserves the exact account key in the %s public snapshot",
  (mode) => {
    const publish = vi.fn();
    if (mode === "screen")
      hooks.screen = createScreenChannel("remote", publish);
    const account = {
      accountKey: "exact-profile",
      provider: "claude" as const,
      email: "work@example.test",
      plan: "Max",
      status: "ready",
      label: "Work",
      isDefault: false,
    };
    const source = { ...account, privateField: "fixture-only" };
    mount(undefined, [source]);
    const calls =
      mode === "screen"
        ? publish.mock.calls
        : surface.parent.postMessage.mock.calls;
    const message = calls.find(
      ([value]) => value.kind === "studio-server-accounts",
    )?.[0];
    expect(message?.serverId).toBe("remote");
    expect(message?.accounts).toEqual([account]);
  },
);

type NavigationMessage = {
  kind: string;
  serverId: string;
  navigation: ServerNavigation;
};
function navigationMessages(): [NavigationMessage, string][] {
  return surface.parent.postMessage.mock.calls
    .filter(([message]) => message.kind === "studio-server-navigation")
    .map(([message, origin]): [NavigationMessage, string] => [message, origin]);
}

it("publishes a real scoped snapshot and republishes after a saved preference edit", () => {
  records.set(
    "codex-project-compact:/same/state",
    JSON.stringify({ "/work": false }),
  );
  records.set(
    ":server:other:codex-project-compact:/same/state",
    JSON.stringify({ "/work": false }),
  );
  mount();
  expect(navigationMessages()).toHaveLength(1);
  const [initial, origin] = required(navigationMessages()[0]);
  expect(origin).toBe("https://parent.example");
  expect(initial.serverId).toBe("remote");
  expect(
    initial.navigation.sidebar?.entities.find(
      (entity) => entity.collection === "agentSummary",
    )?.value.name,
  ).toBe("Real chat");
  expect(initial.navigation.sidebar?.compact).toEqual({});
  expect(initial.navigation.chats[0]).toMatchObject({
    kind: "chat",
    agent: { operationalStatus: { reason: "unread_result" } },
  });
  expect(JSON.stringify(initial.navigation.sidebar)).not.toContain(
    "session-token",
  );

  writePreferenceEdit(
    "codex-project-compact:/same/state",
    {},
    { "/work": true },
  );
  expect(navigationMessages()).toHaveLength(2);
  expect(navigationMessages().at(-1)?.[0].navigation.sidebar?.compact).toEqual({
    "/work": true,
  });
  const folder = JSON.stringify(["/work", "folder", "nested"]);
  writePreferenceEdit("codex-project-tree:/same/state", {}, { [folder]: true });
  expect(navigationMessages()).toHaveLength(3);
  expect(
    navigationMessages().at(-1)?.[0].navigation.sidebar?.collapsed,
  ).toEqual({ [folder]: true });
  surface.dispatchEvent(
    new CustomEvent(preferenceEvent, {
      detail: "codex-project-tree:/other/state",
    }),
  );
  surface.dispatchEvent(
    new TestStorageEvent("storage", {
      key: ":server:other:codex-project-tree:/same/state",
    }),
  );
  expect(navigationMessages()).toHaveLength(3);
});

it("republishes saved order and tree changes from scoped or materialized storage events", () => {
  mount();
  records.set(
    ":server:remote:codex-sidebar-order:/same/state",
    JSON.stringify({ projects: ["/work"] }),
  );
  surface.dispatchEvent(
    new TestStorageEvent("storage", {
      key: ":server:remote:codex-sidebar-order:/same/state",
    }),
  );
  expect(navigationMessages()).toHaveLength(2);
  expect(
    navigationMessages().at(-1)?.[0].navigation.sidebar?.savedOrder,
  ).toEqual({ projects: ["/work"] });
  expect(
    navigationMessages()
      .at(-1)?.[0]
      .navigation.sidebar?.entities.find(
        (entity) => entity.collection === "workspace",
      )?.value,
  ).toEqual(expect.objectContaining({ stateDir: "/same/state" }));
  preferenceMemory.setItem(
    ":server:remote:codex-project-tree:/same/state",
    JSON.stringify({ "/work": true }),
  );
  surface.dispatchEvent(
    new TestStorageEvent("storage", { key: "codex-project-tree:/same/state" }),
  );
  expect(navigationMessages()).toHaveLength(3);
  expect(
    navigationMessages().at(-1)?.[0].navigation.sidebar?.collapsed,
  ).toEqual({ "/work": true });
  records.clear();
  surface.dispatchEvent(new TestStorageEvent("storage"));
  expect(navigationMessages()).toHaveLength(4);
  expect(
    navigationMessages().at(-1)?.[0].navigation.sidebar?.savedOrder,
  ).toEqual({});
});

it("removes preference and storage listeners when the frame effect stops", () => {
  mount();
  cleanup.forEach((stop) => stop());
  cleanup = [];
  surface.dispatchEvent(
    new CustomEvent(preferenceEvent, {
      detail: "codex-project-tree:/same/state",
    }),
  );
  surface.dispatchEvent(
    new TestStorageEvent("storage", { key: "codex-project-tree:/same/state" }),
  );
  expect(navigationMessages()).toHaveLength(1);
  expect(stopConnection).toHaveBeenCalledOnce();
});

it("keeps navigation without sidebar data until a snapshot is available", () => {
  mount(null);
  surface.dispatchEvent(
    new CustomEvent(preferenceEvent, {
      detail: "codex-project-tree:/same/state",
    }),
  );
  expect(navigationMessages()).toHaveLength(1);
  expect(required(navigationMessages()[0])[0].navigation).toMatchObject({
    ready: false,
  });
  expect(required(navigationMessages()[0])[0].navigation).not.toHaveProperty(
    "sidebar",
  );
});

it.each(["parent", "screen"])(
  "publishes native state facts without a Snapshot render through %s",
  (mode) => {
    const publish = vi.fn();
    if (mode === "screen")
      hooks.screen = createScreenChannel("remote", publish);
    mount();
    const calls =
      mode === "screen"
        ? publish.mock.calls
        : surface.parent.postMessage.mock.calls;
    const messages = () =>
      calls.filter(([message]) => message.kind === "studio-server-navigation");
    const mirror = required(hooks.mirror);
    expect(messages()).toHaveLength(1);
    mirror.apply([
      {
        collection: "agentSummary",
        id: "lead",
        value: agentSummaryFixture("lead", {
          name: "Updated from stream",
          operationalStatus: { reason: "needs_input", wait: null },
        }),
      },
    ]);
    expect(messages()).toHaveLength(2);
    expect(messages().at(-1)?.[0].navigation.chats[0]).toMatchObject({
      kind: "chat",
      agent: {
        name: "Updated from stream",
        operationalStatus: { reason: "needs_input" },
      },
    });
    cleanup.forEach((stop) => stop());
    cleanup = [];
    mirror.apply([]);
    expect(messages()).toHaveLength(2);
  },
);
it("captures row-only facts and independent tails as plain native entities", () => {
  const mirror = required(hooks.mirror);
  mount();
  const initial = required(navigationMessages()[0])[0];
  mirror.apply([
    {
      collection: "agentSummary",
      id: "lead",
      value: agentSummaryFixture("lead", { projectFolderRevision: 7 }),
    },
    {
      collection: "agentTail",
      id: "lead",
      value: { id: "lead", tail: "Latest tail" },
    },
  ]);
  expect(navigationMessages()).toHaveLength(2);
  expect(
    initial.navigation.sidebar?.entities.find(
      (entity) => entity.collection === "agentSummary",
    )?.value.projectFolderRevision,
  ).toBe(0);
  expect(navigationMessages().at(-1)?.[0].navigation.sidebar?.entities).toEqual(
    expect.arrayContaining([
      {
        collection: "agentTail",
        id: "lead",
        value: { id: "lead", tail: "Latest tail" },
      },
    ]),
  );
});

it("ignores retired parent commands while the screen channel owns execution", async () => {
  const run = mount();
  surface.dispatchEvent(
    Object.assign(new Event("message"), {
      source: surface.parent,
      origin: "https://parent.example",
      data: {
        kind: "studio-server-command",
        serverId: "remote",
        command: { action: "open", id: "lead" },
      },
    }),
  );
  expect(run).not.toHaveBeenCalled();
});

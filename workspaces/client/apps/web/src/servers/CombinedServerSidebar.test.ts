import { stateFromFixture } from "./sidebarSourceFixture.ts";
import { createSidebarSourceStore } from "./sidebarSourceFixture.ts";
import type { CombinedSidebarServices } from "./combinedSidebarServices.ts";
import type { ApiPostPath, PostInput, PostResult } from "../api.ts";
import type { SidebarBackend } from "../components/sidebar/support/services.tsx";
import type { SidebarCommands } from "../components/sidebar/support/commands.ts";
// @vitest-environment happy-dom
import { act, createElement, useMemo } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { createSidebarList } from "../components/sidebar/support/viewModel.ts";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, expect, it, vi } from "vitest";
import {
  createCombinedSidebarFixture,
  CombinedSidebarFixtureView,
} from "./CombinedServerSidebar.fixture.tsx";
import type { SidebarListOwner } from "../app/shell/SidebarOwner.ts";
import type { ServerCommand } from "./navigation.ts";
import { createMergedSidebarSelector, mergeSidebar } from "./mergedSidebar.ts";
import { sidebarSnapshot } from "./sidebarSnapshot.ts";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";
import {
  agentEntities,
  withSidebarCollections,
} from "./sidebarSourceFixture.ts";
import { componentSnapshot } from "../components/conversation/ComposerAttachments/componentTestFixtures.ts";
const snapshotFixture = (...args: Parameters<typeof componentSnapshot>) =>
  Object.assign(componentSnapshot(...args), {
    token: "",
    stateDir: "/fixture",
  });
import { navigationSnapshot } from "./navigation.ts";
import { localSidebarBackend } from "../components/sidebar/support/services.tsx";
import { required } from "../components/conversation/ComposerAttachments/componentTestFixtures.ts";

const captured = vi.hoisted(() => ({
  owner: null as SidebarListOwner | null,
  props: null as {
    rows: SidebarListOwner["model"]["rows"];
    sidebar: SidebarListOwner["runtime"];
  } | null,
  commands: null as Pick<
    SidebarCommands,
    "open" | "changeProject" | "markUnread"
  > | null,
  lists: 0,
  groups: null as unknown,
  ordered: null as unknown,
}));
vi.mock("../components/sidebar/Sidebar/Sidebar.tsx", () => ({
  default: observer(({ owner }: { owner: SidebarListOwner }) => {
    captured.owner = owner;
    const props = { rows: owner.model.rows, sidebar: owner.runtime };
    captured.props = props;
    captured.commands = {
      open: (id) => {
        void owner.run({ action: "open", id });
      },
      changeProject: (id) => {
        void owner.run({ action: "change-project", id });
      },
      markUnread: (id, target) => {
        void owner.run({ action: "mark-unread", id, target });
      },
    };
    const list = useMemo(() => {
      captured.lists++;
      return createSidebarList(
        () => props.rows || [],
        () => ({ query: "", archived: false, opened: null, rank: () => 0 }),
      );
    }, [props.rows]);
    captured.groups = list.groups;
    captured.ordered = list.orderedRows;
    return null;
  }),
}));

it("routes overlapping row ids and the completed-result CAS target to their owning server", async () => {
  const target = {
    threadId: "native-thread",
    turnId: "completed-turn",
    read: true,
    revision: 7,
  };
  const rows = [
    agentSummaryFixture("same", { markRead: target, markUnread: "allowed" }),
  ];
  const model = mergeSidebar(
    ["local", "remote"].map((id) =>
      withSidebarCollections({
        id,
        online: true,
        sidebar: sidebarSnapshot(
          stateFromFixture(snapshotFixture([])),
          agentEntities(rows),
        ),
      }),
    ),
  );
  const send = vi.fn(),
    executeCommand = vi.fn(async () => {});
  const props = renderCombined(model, executeCommand, send);
  expect(props.rows?.map((row) => row.agent)).toEqual(
    model.rows.map((row) => row.agent),
  );
  expect(props.sidebar).toBe(model.sidebar);
  const id = model.chatKey("remote", "same");
  required(captured.commands).open(id);
  required(captured.commands).changeProject(id);
  required(captured.commands).markUnread(id, target);
  expect(send.mock.calls).toEqual([
    ["remote", { action: "open", id: "same" }],
    ["remote", { action: "change-project", id: "same" }],
  ]);
  expect(executeCommand).toHaveBeenCalledExactlyOnceWith("remote", {
    action: "mark-unread",
    id: "same",
    target,
  });
  await Promise.resolve();
});

function renderCombined(
  model: ReturnType<typeof mergeSidebar>,
  executeCommand: (owner: string, command: ServerCommand) => Promise<void>,
  send: (owner: string, command: ServerCommand) => void = vi.fn(),
) {
  renderToStaticMarkup(combinedElement(model, executeCommand, send));
  return required(captured.props);
}

let fixture: ReturnType<typeof createCombinedSidebarFixture> | undefined;
let stopFixture = () => {};
afterEach(() => {
  stopFixture();
  fixture = undefined;
});
function combinedElement(
  model: ReturnType<typeof mergeSidebar>,
  executeCommand: (owner: string, command: ServerCommand) => Promise<void>,
  send: (owner: string, command: ServerCommand) => void = vi.fn(),
  overrides: Partial<CombinedSidebarServices> = {},
) {
  const services: CombinedSidebarServices = {
    backends: new Map(
      ["local", "remote"].map((id) => [
        id,
        { ...localSidebarBackend, ownerId: id },
      ]),
    ),
    online: () => true,
    refreshOwner: async () => navigationSnapshot(undefined, null, ""),
    executeCommand,
    aliases: {},
    send,
    applyNavigation: vi.fn(),
    settings: vi.fn(),
    search: vi.fn(),
    notify: vi.fn(),
    ...overrides,
  };
  const input = {
    model,
    current: "remote",
    navigation: {},
    mobile: false,
    hidden: false,
    close: vi.fn(),
    notice: "",
    hideOldChatsThreshold: 3,
  };
  if (!fixture) {
    fixture = createCombinedSidebarFixture(input, services);
    stopFixture = fixture.start();
  } else fixture.update(input, services);
  return createElement(CombinedSidebarFixtureView, { fixture });
}

it("freezes a queued read CAS target while its generated row mirror advances", async () => {
  const initial = {
    threadId: "native",
    turnId: "original-turn",
    read: true,
    revision: 7,
  };
  const source = {
    id: "remote",
    online: true,
    sidebar: sidebarSnapshot(
      stateFromFixture(snapshotFixture([])),
      agentEntities([
        agentSummaryFixture("same", {
          markRead: initial,
          markUnread: "allowed",
        }),
      ]),
    ),
  };
  const store = createSidebarSourceStore();
  store.update("remote", source.sidebar);
  const sources = () => store.sources(["remote"], new Set(["remote"]));
  const select = createMergedSidebarSelector();
  const model = select(sources());
  const target = required(required(model.rows[0]).agent.markRead);
  let release = () => {};
  const ready = new Promise<void>((resolve) => {
    release = resolve;
  });
  const delivered: ServerCommand[] = [];
  let dispatch: Promise<void> | undefined;
  renderCombined(model, (_owner, command) => {
    dispatch = ready.then(() => {
      delivered.push(command);
    });
    return dispatch;
  });
  required(captured.commands).markUnread(
    model.chatKey("remote", "same"),
    target,
  );
  store.update(
    "remote",
    sidebarSnapshot(
      stateFromFixture(snapshotFixture([])),
      agentEntities([
        agentSummaryFixture("same", {
          markRead: {
            threadId: "native",
            turnId: "newer-turn",
            read: false,
            revision: 8,
          },
        }),
      ]),
    ),
  );
  select(sources());
  expect(target.turnId).toBe("newer-turn");
  expect(delivered).toEqual([]);
  release();
  await required(dispatch);
  expect(delivered).toEqual([
    { action: "mark-unread", id: "same", target: initial },
  ]);
});

it("preserves combined list computations on a tail rerender and replaces rows once per source change", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const store = createSidebarSourceStore();
  const snapshot = (tail = "") =>
    sidebarSnapshot(stateFromFixture(snapshotFixture([])), [
      ...agentEntities([agentSummaryFixture("same")]),
      { collection: "agentTail", id: "same", value: { id: "same", tail } },
    ]);
  store.update("local", snapshot());
  const select = createMergedSidebarSelector();
  let ids = ["local"];
  const sources = () => store.sources(ids, new Set(ids));
  const host = document.createElement("div");
  const root = createRoot(host);
  const render = () =>
    root.render(
      combinedElement(
        select(sources()),
        vi.fn(async () => {}),
      ),
    );
  try {
    await act(async () => {
      render();
    });
    const rows = required(captured.props).rows;
    const groups = captured.groups,
      ordered = captured.ordered,
      lists = captured.lists;
    await act(async () => {
      store.update("local", snapshot("tail-only update"));
      render();
    });
    expect(required(captured.props).rows).toBe(rows);
    expect(captured.groups).toBe(groups);
    expect(captured.ordered).toBe(ordered);
    expect(captured.lists).toBe(lists);
    await act(async () => {
      store.update("remote", snapshot());
      ids = ["local", "remote"];
      render();
    });
    expect(required(captured.props).rows).not.toBe(rows);
    expect(captured.lists).toBe(lists + 1);
    const added = required(captured.props).rows;
    await act(async () => {
      ids = ["local"];
      render();
    });
    expect(required(captured.props).rows).not.toBe(added);
    expect(captured.lists).toBe(lists + 2);
  } finally {
    await act(async () => root.unmount());
    vi.unstubAllGlobals();
  }
});

it("routes project and folder actions through their wire owners and refuses offline mutations", () => {
  const model = mergeSidebar(
    ["local", "remote"].map((id) =>
      withSidebarCollections({
        id,
        online: true,
        sidebar: {
          ...sidebarSnapshot(stateFromFixture(snapshotFixture([])), []),
          stateDir: "shared",
          rooms: [],
          projects: [
            {
              id: `${id}-project`,
              path: `/${id}`,
              folders: [{ id: "folder", name: id }],
            },
          ],
          entities: agentEntities([
            agentSummaryFixture("same", {
              projectPath: `/${id}`,
              projectFolder: "folder",
            }),
          ]),
        },
      }),
    ),
  );
  const send = vi.fn(),
    notify = vi.fn(),
    settings = vi.fn(),
    search = vi.fn();
  let online = true;
  renderToStaticMarkup(
    combinedElement(
      model,
      vi.fn(async () => {}),
      send,
      { online: () => online, notify, settings, search },
    ),
  );
  const owner = required(captured.owner),
    path = model.projectKey("remote", "/remote"),
    folder = model.folderKey("remote", "/remote", "folder");
  const commands: ServerCommand[] = [
    { action: "new-chat", path, folder },
    { action: "new-shared-chat", path },
    { action: "project-account", path },
    { action: "project-folders", projectId: path },
    { action: "delete-project", projectId: path },
    { action: "remove-chat", id: model.chatKey("remote", "same"), room: false },
  ];
  for (const command of commands) void owner.run(command);
  expect(send.mock.calls).toEqual([
    [
      "remote",
      {
        action: "new-chat",
        path: "/remote",
        folder: "folder",
        projectId: "remote-project",
        projectServerId: "remote",
      },
    ],
    ["remote", { action: "new-shared-chat", path: "/remote" }],
    ["remote", { action: "project-account", path: "/remote" }],
    ["remote", { action: "project-folders", projectId: "/remote" }],
    ["remote", { action: "delete-project", projectId: "/remote" }],
    ["remote", { action: "remove-chat", id: "same", room: false }],
  ]);
  send.mockClear();
  online = false;
  for (const command of commands) void owner.run(command);
  void owner.run({ action: "new-chat" });
  void owner.run({ action: "add-project" });
  expect(send).not.toHaveBeenCalled();
  expect(notify).toHaveBeenCalledTimes(commands.length + 2);
  void owner.run({ action: "settings" });
  void owner.run({ action: "search", query: "", requestId: "search" });
  expect(settings).toHaveBeenCalledOnce();
  expect(search).toHaveBeenCalledOnce();
});

it("retains the exact rename across a lost reply and settles without a refresh", async () => {
  const model = mergeSidebar(
    ["local", "remote"].map((id) =>
      withSidebarCollections({
        id,
        online: true,
        sidebar: sidebarSnapshot(
          stateFromFixture(snapshotFixture([])),
          agentEntities([agentSummaryFixture("same")]),
        ),
      }),
    ),
  );
  const values = new Map<string, string>(),
    requests: unknown[] = [];
  let transportFailure = true;
  const backend: SidebarBackend = {
    ...localSidebarBackend,
    ownerId: "remote",
    storage: {
      getItem: (key) => values.get(key) ?? null,
      setItem: (key, value) => {
        values.set(key, value);
      },
      removeItem: (key) => {
        values.delete(key);
      },
    },
    saved<T>(key: string, fallback: T): T {
      const value = values.get(key);
      return value === undefined ? fallback : JSON.parse(value);
    },
    async post<Path extends ApiPostPath>(
      _path: Path,
      body: PostInput<Path>,
    ): Promise<PostResult<Path>> {
      requests.push(body);
      if (transportFailure) throw Error("Lost reply");
      return {} as PostResult<Path>;
    },
  };
  const backends = new Map([
    ["local", { ...backend, ownerId: "local" }],
    ["remote", backend],
  ]);
  renderToStaticMarkup(
    combinedElement(
      model,
      vi.fn(async () => {}),
      vi.fn(),
      {
        backends,
        refreshOwner: async () => {
          throw Error("Unexpected refresh");
        },
      },
    ),
  );
  const owner = required(captured.owner),
    id = model.chatKey("remote", "same");
  await expect(owner.renameChat(id, "Saved name")).rejects.toThrow(
    "Lost reply",
  );
  transportFailure = false;
  await owner.renameChat(id, "Saved name");
  expect(requests).toHaveLength(2);
  expect(requests[1]).toEqual(requests[0]);
  expect(requests[0]).toMatchObject({ id: "same", name: "Saved name" });
  expect(requests).toHaveLength(2);
  expect(values.has("studio-sidebar-rename:same")).toBe(false);
  const replacementRequests: unknown[] = [];
  backends.set("remote", {
    ...backend,
    async post<Path extends ApiPostPath>(
      _path: Path,
      body: PostInput<Path>,
    ): Promise<PostResult<Path>> {
      replacementRequests.push(body);
      return {} as PostResult<Path>;
    },
  });
  await owner.renameChat(id, "Replacement backend");
  expect(requests).toHaveLength(2);
  expect(replacementRequests).toHaveLength(1);
  expect(replacementRequests[0]).toMatchObject({
    id: "same",
    name: "Replacement backend",
  });
});

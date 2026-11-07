import {
  useSidebarCommands,
  type SidebarCommands,
} from "../components/sidebar/commands.ts";
import { CombinedSidebarServicesProvider } from "./combinedSidebarServices.ts";
// @vitest-environment happy-dom
import { act, createElement, useMemo } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { createSidebarList } from "../components/sidebar/viewModel.ts";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it, vi } from "vitest";
import type { SidebarProps } from "../components/Sidebar.tsx";
import CombinedServerSidebar from "./CombinedServerSidebar.tsx";
import type { ServerCommand } from "./navigation.ts";
import { createMergedSidebarSelector, mergeSidebar } from "./mergedSidebar.ts";
import { sidebarSnapshot } from "./sidebarSnapshot.ts";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";
import {
  agentEntities,
  withSidebarCollections,
} from "./sidebarSourceFixture.ts";
import { createSidebarSourceStore } from "./mergedSidebar.ts";
import { snapshotFixture } from "../snapshotFixture.ts";
import { navigationSnapshot } from "./navigation.ts";
import { localSidebarBackend } from "../components/sidebar/services.tsx";
import { required } from "../components/componentTestFixtures.ts";

const captured = vi.hoisted(() => ({
  props: null as SidebarProps | null,
  commands: null as SidebarCommands | null,
  lists: 0,
  groups: null as unknown,
  ordered: null as unknown,
}));
vi.mock("../components/Sidebar", () => ({
  default: observer((props: SidebarProps) => {
    captured.props = props;
    captured.commands = useSidebarCommands();
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
        sidebar: sidebarSnapshot(snapshotFixture([]), agentEntities(rows)),
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

function combinedElement(
  model: ReturnType<typeof mergeSidebar>,
  executeCommand: (owner: string, command: ServerCommand) => Promise<void>,
  send: (owner: string, command: ServerCommand) => void = vi.fn(),
) {
  return createElement(
    CombinedSidebarServicesProvider,
    {
      value: {
        backends: new Map(
          ["local", "remote"].map((id) => [
            id,
            { ...localSidebarBackend, ownerId: id },
          ]),
        ),
        online: () => true,
        refreshOwner: async () => navigationSnapshot(null, null, ""),
        executeCommand,
        aliases: {},
        send,
        applyNavigation: vi.fn(),
        settings: vi.fn(),
        search: vi.fn(),
        notify: vi.fn(),
      },
    },
    createElement(CombinedServerSidebar, {
      model,
      current: "remote",
      navigation: {},
      mobile: false,
      hidden: false,
      close: vi.fn(),
      notice: "",
      hideOldChatsThreshold: 3,
    }),
  );
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
      snapshotFixture([]),
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
      snapshotFixture([]),
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
    sidebarSnapshot(snapshotFixture([]), [
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

import { action, autorun, observable } from "mobx";
import { createApiStorage } from "../../../../apiStorage.ts";
// @vitest-environment happy-dom
import { RoomKind } from "../../../../generated/api.ts";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../integration-support/observableFixture.tsx";
import { ServerAreasContextProvider as ConversationProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { agentSummaryFixture } from "../../../../servers/agentSummaryFixture.ts";
import type { StateEntity } from "../../../../generated/observableResources.ts";
import { serverLocalStorage } from "../../../../servers/storage.ts";

const realRoom = vi.hoisted(() => ({
  enabled: false,
  syncGet: vi.fn(),
  save: vi.fn(),
}));
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => {
    const actual =
      await original<
        typeof import("../../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
      >();
    const api = {
      syncGet: realRoom.syncGet,
      saved: (_key: string, fallback: unknown) => fallback,
      save: realRoom.save,
    };
    return {
      ...actual,
      useServerApi: () => (realRoom.enabled ? api : actual.useServerApi()),
    };
  },
);
import { RoomHistory } from "../../../../conversation/roomHistory.ts";
import TeamChats from "./TeamChats.tsx";
import { createTeamChatsStore } from "./TeamChatsStore.ts";
import { resourceData } from "../../../../store/runtime.ts";
function messagesOwner(f: ReturnType<typeof observableFixture>) {
  return createTeamChatsStore(
    realRoom.enabled
      ? f.store
      : { ...f.store, room: () => ({ current: { status: "absent" } }) },
    {
      saved: (_key, fallback) => fallback,
      save: realRoom.save,
      syncGet: realRoom.syncGet,
    },
    "",
    undefined,
    () => ({
      stateDir:
        resourceData(f.store.state().current)?.workspaces.values().next().value
          ?.stateDir || "",
      leadId: "lead",
    }),
  );
}

it("room content updates isolate siblings and search never renders open detail", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const f = observableFixture();
  const actions = conversationActionsFixture();
  const errors = vi.spyOn(console, "error");
  const historyStart = vi.spyOn(RoomHistory.prototype, "start");
  const owner = messagesOwner(f);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const counts = new Map<string, number>();
  window.__studioMessageRenderProbe = (leaf, id) => {
    const key = leaf + ":" + (id || "");
    counts.set(key, (counts.get(key) || 0) + 1);
  };
  const entities: StateEntity[] = [
    {
      collection: "workspace",
      id: "w",
      value: { stateDir: "messages-isolation" },
    },
    ...["lead", "a", "b"].map((id) => ({
      collection: "agentSummary" as const,
      id,
      value: {
        ...agentSummaryFixture(id),
        rootId: id === "lead" ? null : "lead",
      },
    })),
    ...["a", "b"].map((id) => ({
      collection: "room" as const,
      id,
      value: {
        id,
        name: "Room " + id,
        kind: "broadcast" as const,
        rootId: "lead",
        lastMessage: { seq: 1, created: 1, sender: id, text: "Preview " + id },
      },
    })),
  ];
  try {
    f.apply(entities);
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          {},
          createElement(
            f.Provider,
            {},
            createElement(
              ConversationProvider,
              { value: actions },
              createElement(TeamChats, {
                owner,
              }),
            ),
          ),
        ),
      ),
    );
    counts.clear();
    await act(async () =>
      f.apply(
        entities.map((entity) =>
          entity.collection === "room" && entity.id === "a"
            ? {
                ...entity,
                value: {
                  ...entity.value,
                  lastMessage: {
                    seq: 1,
                    created: 1,
                    sender: "a",
                    text: "Changed preview",
                  },
                },
              }
            : entity,
        ),
      ),
    );
    expect(host.textContent).toContain("Changed preview");
    expect(counts.get("room-row:a")).toBe(1);
    expect(counts.get("room-row:b") || 0).toBe(0);
    expect(counts.get("personal-row:") || 0).toBe(0);
    expect(counts.get("personal-feed:") || 0).toBe(0);
    expect(counts.get("team-detail:you") || 0).toBe(0);
    const room = host.querySelector<HTMLElement>('[data-room="b"]');
    expect(room).not.toBeNull();
    await act(async () => room?.click());
    const detail = host.querySelector(".team-room-detail");
    counts.clear();
    const activeHistory = owner.messages;
    const before = historyStart.mock.calls.length;
    const input = host.querySelector<HTMLInputElement>(
      '[aria-label="Search chats"]',
    );
    if (!input) throw new Error("Search input missing");
    await act(async () => {
      Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )?.set?.call(input, "Changed");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    expect(input.value).toBe("Changed");
    expect(host.querySelector('[data-room="b"]')).toBeNull();
    expect(host.querySelector(".team-room-detail")).toBe(detail);
    expect(counts.get("team-detail:b") || 0).toBe(0);
    expect(owner.messages).toBe(activeHistory);
    expect(historyStart).toHaveBeenCalledTimes(before);
    expect(
      serverLocalStorage.getItem("codex-chat-seen:messages-isolation"),
    ).toBeNull();
    vi.stubGlobal("innerWidth", 600);
    await act(async () => owner.back());
    vi.stubGlobal("innerWidth", 1000);
    await act(async () => window.dispatchEvent(new Event("resize")));
    expect(host.querySelector(".team-room-detail h2")?.textContent).toBe(
      "For you",
    );
    expect(owner.messages).not.toBe(activeHistory);
  } finally {
    await act(async () => root.unmount());
    const renderWarnings = errors.mock.calls.filter((args) =>
      args.some((arg) => String(arg).includes("Cannot update a component")),
    );
    errors.mockRestore();
    historyStart.mockRestore();
    f.store.dispose();
    host.remove();
    delete window.__studioMessageRenderProbe;
    vi.unstubAllGlobals();
    expect(renderWarnings).toEqual([]);
  }
});

import { createRoomMirror } from "../../../../generated/observableResources.ts";
it("can request an older page after selecting another room and returning", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  realRoom.enabled = true;
  const f = observableFixture();
  const dto = {
    room: { id: "a" },
    messages: [
      {
        id: "m",
        seq: 2,
        room: "a",
        sender: "lead",
        senderName: "Lead",
        created: 1,
        text: "Message",
        deliveries: {},
      },
    ],
    nextBefore: 2,
  };
  const mirror = createRoomMirror(dto);
  vi.spyOn(f.store, "room").mockImplementation(() => ({
    get current() {
      return { status: "live" as const, data: mirror.current };
    },
  }));
  const pending = Promise.withResolvers<typeof dto>();
  realRoom.syncGet.mockReturnValueOnce(pending.promise).mockResolvedValue(dto);
  f.apply([
    {
      collection: "workspace",
      id: "w",
      value: { stateDir: "room-page-reentry" },
    },
    {
      collection: "agentSummary",
      id: "lead",
      value: agentSummaryFixture("lead"),
    },
    ...["a", "b"].map((id) => ({
      collection: "room" as const,
      id,
      value: {
        id,
        name: "Room " + id,
        kind: "broadcast" as const,
        rootId: "lead",
      },
    })),
  ]);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const click = async (selector: string) => {
    const el = host.querySelector<HTMLElement>(selector);
    expect(el).not.toBeNull();
    await act(async () => el!.click());
  };
  const older = async () => {
    const el = [...host.querySelectorAll("button")].find(
      (e) => e.textContent === "Earlier messages",
    );
    expect(el).toBeDefined();
    await act(async () => el!.click());
  };
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          {},
          createElement(
            f.Provider,
            {},
            createElement(
              ConversationProvider,
              { value: conversationActionsFixture() },
              createElement(TeamChats, {
                owner: messagesOwner(f),
              }),
            ),
          ),
        ),
      ),
    );
    await click('[data-room="a"]');
    await older();
    await click('[data-room="b"]');
    await click('[data-room="a"]');
    await older();
    expect(realRoom.syncGet).toHaveBeenCalledTimes(2);
  } finally {
    await act(async () => {
      pending.resolve(dto);
      await pending.promise;
      root.unmount();
    });
    realRoom.enabled = false;
    realRoom.syncGet.mockReset();
    f.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});

it("failed room history leaves its unread position unacknowledged", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  realRoom.enabled = true;
  realRoom.save.mockClear();
  const f = observableFixture();
  vi.spyOn(f.store, "room").mockReturnValue({
    current: { status: "error", error: new Error("History unavailable") },
  });
  f.apply([
    { collection: "workspace", id: "w", value: { stateDir: "failed-history" } },
    {
      collection: "agentSummary",
      id: "lead",
      value: agentSummaryFixture("lead"),
    },
    {
      collection: "room",
      id: "failed",
      value: {
        id: "failed",
        name: "Room",
        kind: RoomKind.broadcast,
        rootId: "lead",
        lastMessage: { seq: 9, created: 1, sender: "lead", text: "Unread" },
      },
    },
  ]);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          {},
          createElement(
            f.Provider,
            {},
            createElement(
              ConversationProvider,
              { value: conversationActionsFixture() },
              createElement(TeamChats, {
                owner: messagesOwner(f),
              }),
            ),
          ),
        ),
      ),
    );
    await act(async () =>
      host.querySelector<HTMLElement>('[data-room="failed"]')!.click(),
    );
    expect(host.textContent).toContain("History unavailable");
    expect(
      host
        .querySelector('[data-room="failed"]')
        ?.getAttribute("data-studio-state"),
    ).toBe("unread");
    expect(realRoom.save).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
    realRoom.enabled = false;
    realRoom.save.mockReset();
    f.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});

it("acknowledges only loaded rows inside the viewport as the room scrolls", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  realRoom.enabled = true;
  realRoom.save.mockClear();
  const f = observableFixture();
  const mirror = createRoomMirror({
    room: { id: "visible" },
    messages: [2, 3].map((seq) => ({
      id: "m" + seq,
      seq,
      room: "visible",
      sender: "lead",
      senderName: "Lead",
      created: 1,
      text: "Message " + seq,
      deliveries: {},
    })),
  });
  vi.spyOn(f.store, "room").mockReturnValue({
    current: { status: "live", data: mirror.current },
  });
  f.apply([
    {
      collection: "workspace",
      id: "w",
      value: { stateDir: "visible-history" },
    },
    {
      collection: "agentSummary",
      id: "lead",
      value: agentSummaryFixture("lead"),
    },
    {
      collection: "room",
      id: "visible",
      value: {
        id: "visible",
        name: "Room",
        kind: RoomKind.broadcast,
        rootId: "lead",
        lastMessage: {
          seq: 9,
          created: 1,
          sender: "lead",
          text: "Not loaded yet",
        },
      },
    },
  ]);
  let scrolled = false;
  const rects = vi
    .spyOn(HTMLElement.prototype, "getBoundingClientRect")
    .mockImplementation(function (this: HTMLElement) {
      if (this.classList.contains("unified-message-scroll"))
        return new DOMRect(0, 0, 200, 100);
      if (this.dataset.roomRow)
        return new DOMRect(
          0,
          (this.dataset.roomRow === "m2" ? 20 : 140) - (scrolled ? 120 : 0),
          200,
          60,
        );
      return new DOMRect();
    });
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          {},
          createElement(
            f.Provider,
            {},
            createElement(
              ConversationProvider,
              { value: conversationActionsFixture() },
              createElement(TeamChats, {
                owner: messagesOwner(f),
              }),
            ),
          ),
        ),
      ),
    );
    await act(async () =>
      host.querySelector<HTMLElement>('[data-room="visible"]')!.click(),
    );
    expect(realRoom.save).toHaveBeenLastCalledWith(
      "codex-chat-seen:visible-history",
      { visible: 2 },
    );
    expect(
      host
        .querySelector('[data-room="visible"]')
        ?.getAttribute("data-studio-state"),
    ).toBe("unread");
    scrolled = true;
    await act(async () =>
      host
        .querySelector(".unified-message-scroll")!
        .dispatchEvent(new Event("scroll")),
    );
    expect(realRoom.save).toHaveBeenLastCalledWith(
      "codex-chat-seen:visible-history",
      { visible: 3 },
    );
    expect(
      host
        .querySelector('[data-room="visible"]')
        ?.getAttribute("data-studio-state"),
    ).toBe("unread");
  } finally {
    await act(async () => root.unmount());
    rects.mockRestore();
    realRoom.enabled = false;
    realRoom.save.mockReset();
    f.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});

it("resets every presentation choice when the scope changes on one store", async () => {
  const f = observableFixture();
  const storage = createApiStorage(serverLocalStorage);
  const scope = observable({ stateDir: "team-a", leadId: "lead-a" });
  f.apply(
    ["a", "b"].map((id) => ({
      collection: "room",
      id,
      value: {
        id,
        name: id === "a" ? "Alpha" : "Beta",
        kind: "broadcast",
        rootId: "lead-" + id,
      },
    })),
  );
  storage.save("codex-chat-seen:team-b", { b: 2 });
  const chats = createTeamChatsStore(
    f.store,
    storage,
    "",
    undefined,
    () => scope,
  );
  const stop = autorun(() => {
    void chats.current;
  });
  try {
    await Promise.resolve();
    chats.search("Alpha");
    chats.more();
    chats.choose("a");
    expect(chats.detail).toBe(true);
    action(() => {
      scope.stateDir = "team-b";
      scope.leadId = "lead-b";
    })();
    expect({
      query: chats.query,
      detail: chats.detail,
      limit: chats.limit,
    }).toEqual({ query: "", detail: false, limit: 60 });
    expect(chats.selected).toBe("you");
    expect(chats.filtered.map((room) => room.id)).toEqual(["b"]);
    expect([...chats.seen]).toEqual([["b", 2]]);
  } finally {
    stop();
    f.store.dispose();
  }
});

it("renders personal-feed requests and acts through the selected conversation request owner", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: new EventTarget(),
  });
  const { requestFixture } =
    await import("../../../conversation/questions/support/requestFixtures.ts");
  const { approval, question } =
    await import("../../../../approvalFixtures.ts");
  const { ServerStoreProvider } =
    await import("../../../servers/ServerServicesProvider/ServerServicesProvider.tsx");
  const f = requestFixture([
    approval({
      id: "feed-question",
      agent: "lead",
      request: {
        kind: "agent_question",
        questions: [question("choice", "Feed answer?")],
      },
    }),
  ]);
  const actions = conversationActionsFixture(() => {}, f.store);
  actions.selected.requestOwner.start();
  const owner = createTeamChatsStore(
    f.store,
    {
      saved: (_key, fallback) => fallback,
      save() {},
      syncGet: realRoom.syncGet.mockResolvedValue({ room: null, messages: [] }),
    },
    "feed",
    undefined,
  );
  owner.choose("you");
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            ServerStoreProvider,
            { value: f.store },
            createElement(
              ConversationProvider,
              { value: actions },
              createElement(TeamChats, { owner }),
            ),
          ),
        ),
      ),
    );
    expect(host.querySelector('[data-request="feed-question"]')).not.toBeNull();
    const view = actions.selected.requestOwner.views.get("feed-question")!;
    await act(async () =>
      host.querySelector<HTMLButtonElement>("[data-answer]")!.click(),
    );
    expect(view.open).toBe(true);
    expect(
      host.querySelector('textarea[aria-label="Feed answer?"]'),
    ).not.toBeNull();
  } finally {
    await act(async () => root.unmount());
    actions.selected.closeConversation();
    actions.scope.services.close();
    f.store.dispose();
  }
});

it("honors focus actions and live scope changes through one explicit owner", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const f = observableFixture();
  const actions = conversationActionsFixture();
  f.apply(
    ["a", "b"].map((id) => ({
      collection: "room",
      id,
      value: {
        id,
        name: "Room " + id,
        kind: RoomKind.broadcast,
        rootId: "lead-" + id,
      },
    })),
  );
  const scope = observable({ stateDir: "prop-scope", leadId: "lead-a" });
  const owner = createTeamChatsStore(
    f.store,
    {
      saved: (_key, fallback) => fallback,
      save() {},
      syncGet: vi.fn().mockResolvedValue({ room: null, messages: [] }),
    },
    "prop-scope",
    "lead-a",
    () => scope,
  );
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const render = () =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            f.Provider,
            null,
            createElement(
              ConversationProvider,
              { value: actions },
              createElement(TeamChats, { owner }),
            ),
          ),
        ),
      ),
    );
  try {
    owner.focus("a", undefined, "request-a");
    await render();
    expect(owner.selected).toBe("a");
    expect(
      host.querySelector('[data-room="a"]')?.getAttribute("aria-pressed"),
    ).toBe("true");
    await act(async () =>
      action(() => {
        scope.leadId = "lead-b";
      })(),
    );
    await act(async () => owner.focus("b", undefined, "request-b"));
    expect(owner.filtered.map((room) => room.id)).toEqual(["b"]);
    expect(owner.selected).toBe("b");
    expect(host.querySelector('[data-room="a"]')).toBeNull();
    await act(async () =>
      owner.focus(undefined, "personal-item", "request-personal"),
    );
    expect(owner.selected).toBe("you");
    expect(owner.detail).toBe(true);
  } finally {
    await act(async () => root.unmount());
    actions.scope.services.close();
    f.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});

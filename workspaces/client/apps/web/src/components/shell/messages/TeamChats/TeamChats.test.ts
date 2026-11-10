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
import { ConversationProvider } from "../../../conversation/support/navigation.ts";
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
const history = vi.hoisted(() =>
  vi.fn(() => ({
    items: [],
    loaded: true,
    before: null,
    after: null,
    setPageAnchor() {},
    showLatest() {},
  })),
);
vi.mock("../../../../conversation/roomHistory.ts", async (original) => {
  const actual =
    await original<typeof import("../../../../conversation/roomHistory.ts")>();
  return {
    ...actual,
    useRoomHistory: (...args: Parameters<typeof actual.useRoomHistory>) =>
      realRoom.enabled ? actual.useRoomHistory(...args) : history(),
  };
});
import TeamChats from "./TeamChats.tsx";

it("room content updates isolate siblings and search never renders open detail", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const f = observableFixture();
  const actions = conversationActionsFixture();
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
              createElement(TeamChats, { leadId: "lead" }),
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
    const before = history.mock.calls.length;
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
    expect(history.mock.calls.length).toBe(before);
    expect(
      serverLocalStorage.getItem("codex-chat-seen:messages-isolation"),
    ).toBeNull();
  } finally {
    await act(async () => root.unmount());
    f.store.dispose();
    host.remove();
    delete window.__studioMessageRenderProbe;
    vi.unstubAllGlobals();
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
              createElement(TeamChats, { leadId: "lead" }),
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
              createElement(TeamChats, { leadId: "lead" }),
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
              createElement(TeamChats, { leadId: "lead" }),
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

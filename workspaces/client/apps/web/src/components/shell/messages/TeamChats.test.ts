// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../observableFixture";
import { ConversationProvider } from "../../conversation/navigation";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture";
import type { StateEntity } from "../../../generated/observableResources";
import { serverLocalStorage } from "../../../servers/storage";

const realRoom = vi.hoisted(() => ({ enabled: false, syncGet: vi.fn() }));
vi.mock("../../../servers/ServerServicesContext", async (original) => {
  const actual =
    await original<typeof import("../../../servers/ServerServicesContext")>();
  const api = {
    syncGet: realRoom.syncGet,
    saved: (_key: string, fallback: unknown) => fallback,
    save: () => {},
  };
  return {
    ...actual,
    useServerApi: () => (realRoom.enabled ? api : actual.useServerApi()),
  };
});
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
vi.mock("../../../conversation/roomHistory", async (original) => {
  const actual =
    await original<typeof import("../../../conversation/roomHistory")>();
  return {
    ...actual,
    useRoomHistory: (...args: Parameters<typeof actual.useRoomHistory>) =>
      realRoom.enabled ? actual.useRoomHistory(...args) : history(),
  };
});
import TeamChats from "./TeamChats";

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
    ).toContain('"b":1');
  } finally {
    await act(async () => root.unmount());
    f.store.dispose();
    host.remove();
    delete window.__studioMessageRenderProbe;
    vi.unstubAllGlobals();
  }
});

import { createRoomMirror } from "../../../generated/observableResources";
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

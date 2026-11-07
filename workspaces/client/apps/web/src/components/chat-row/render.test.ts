import { ChatOperationalKind } from "../../generated/api.ts";
// @vitest-environment happy-dom
import { act, createElement, Profiler, StrictMode } from "react";
import { MantineProvider } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { expect, it, vi } from "vitest";
import type { SidebarChatRow } from "../../generated/observableResources.ts";
import { sidebarRowFixture } from "../../servers/sidebarRowFixture.ts";
import { ChatTitle, ChatIndicator, ChatCopyProgress } from "./index.ts";
import { fixture } from "./testFixture.ts";

it.each([false, true])(
  "isolates generated title, indicator and progress leaves; equal replay is silent (StrictMode=%s)",
  async (strict) => {
    const f = fixture();
    const commits = new Map<string, number>();
    const count = (id: string) => commits.set(id, (commits.get(id) || 0) + 1);
    let parentRenders = 0;
    let conversationRenders = 0;
    function Conversation() {
      conversationRenders++;
      return createElement("p", null, "Conversation");
    }
    const Rows = observer(function Rows() {
      parentRenders++;
      const slot = f.store.sidebar().current;
      if (slot.status !== "live" && slot.status !== "stale") return null;
      return slot.data.rows.map((row) =>
        createElement(
          "div",
          { key: row.id },
          createElement(
            Profiler,
            { id: `title:${row.id}`, onRender: count },
            createElement(ChatTitle, { row }),
          ),
          createElement(
            Profiler,
            { id: `indicator:${row.id}`, onRender: count },
            createElement(ChatIndicator, { row }),
          ),
          createElement(
            Profiler,
            { id: `progress:${row.id}`, onRender: count },
            createElement(ChatCopyProgress, { row }),
          ),
        ),
      );
    });
    const tree = createElement(
      MantineProvider,
      null,
      createElement(Rows),
      createElement(Conversation),
    );
    await act(async () =>
      f.root.render(strict ? createElement(StrictMode, null, tree) : tree),
    );
    await act(async () => vi.advanceTimersByTimeAsync(100));
    let rows: readonly SidebarChatRow[] = [
      sidebarRowFixture("a"),
      sidebarRowFixture("b", {
        copyProgress: { phase: "staging", done: 1, total: 10, unit: "files" },
      }),
    ];
    const publish = async () =>
      act(async () => {
        f.publish(rows);
        await vi.advanceTimersByTimeAsync(0);
      });
    await publish();
    const parentBefore = parentRenders;
    const conversationBefore = conversationRenders;
    for (const [field, patch] of [
      [
        "indicator",
        {
          indicator: { kind: ChatOperationalKind["working"], label: "Working" },
        },
      ],
      ["title", { name: "Changed title" }],
      [
        "progress",
        {
          copyProgress: { phase: "staging", done: 2, total: 10, unit: "files" },
        },
      ],
    ] satisfies readonly (readonly [string, Partial<SidebarChatRow>])[]) {
      commits.clear();
      rows = rows.map((row) => (row.id === "b" ? { ...row, ...patch } : row));
      await publish();
      expect([...commits]).toEqual([[`${field}:b`, 1]]);
    }
    commits.clear();
    rows = rows.map((row) => ({
      ...row,
      indicator: { ...row.indicator },
      copyProgress: row.copyProgress ? { ...row.copyProgress } : null,
    }));
    await publish();
    expect([...commits]).toEqual([]);
    expect(parentRenders).toBe(parentBefore);
    expect(conversationRenders).toBe(conversationBefore);
    expect(f.added).toHaveBeenCalledTimes(1);
    await f.close();
  },
);

it("retires the optimistic unread overlay when the generated CAS target changes", async () => {
  const f = fixture();
  const target = {
    read: false,
    threadId: "thread",
    turnId: "turn",
    revision: 2,
  };
  const Rows = observer(function Rows() {
    const slot = f.store.sidebar().current;
    if (slot.status !== "live" && slot.status !== "stale") return null;
    const row = slot.data.rows[0];
    if (!row) throw new Error("Missing sidebar row fixture");
    return createElement(ChatIndicator, {
      row,
      pendingRead: target,
    });
  });
  await act(async () =>
    f.root.render(createElement(MantineProvider, null, createElement(Rows))),
  );
  await act(async () => vi.advanceTimersByTimeAsync(100));
  const row = sidebarRowFixture("a", {
    indicator: { kind: ChatOperationalKind["unread"], label: "Unread result" },
    markRead: target,
  });
  await act(async () => {
    f.publish([row]);
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(document.querySelector("[data-chat-status]")).toBeNull();
  for (const patch of [
    { revision: 3 },
    { turnId: "new-turn" },
    { threadId: "new-thread" },
  ]) {
    await act(async () => {
      f.publish([{ ...row, markRead: { ...target, ...patch } }]);
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(
      document
        .querySelector("[data-chat-status]")
        ?.getAttribute("data-chat-status"),
    ).toBe("unread");
  }
  await f.close();
});

it("keeps row attributes current and dispatches the latest caller-owned actions", async () => {
  const f = fixture();
  const { default: SidebarRow } = await import("../SidebarRow.tsx");
  const { actions } = await import("./testFixture.ts");
  const firstOpen = vi.fn();
  const nextOpen = vi.fn();
  const nextUnread = vi.fn();
  const target = {
    read: true,
    threadId: "thread",
    turnId: "turn",
    revision: 2,
  };
  const row = sidebarRowFixture("a", {
    name: "A chat",
    pinned: true,
    markRead: target,
    markUnread: "allowed",
    indicator: { kind: ChatOperationalKind["working"], label: "Working" },
  });
  const render = async (open: () => void) =>
    act(async () =>
      f.root.render(
        createElement(
          MantineProvider,
          null,
          createElement(SidebarRow, {
            row,
            selected: true,
            serverAlias: "Remote",
            showServer: true,
            renaming: false,
            organizing: false,
            name: "",
            compact: false,
            markingRead: false,
            bindings: {},
            actions: { ...actions, open, unread: nextUnread },
          }),
        ),
      ),
    );
  await render(firstOpen);
  await render(nextOpen);
  const button = document.querySelector<HTMLButtonElement>("button.chat-row");
  expect(button?.title).toBe("A chat");
  expect(button?.getAttribute("aria-description")).toContain("Working");
  expect(
    document
      .querySelector("[data-studio-state]")
      ?.getAttribute("data-studio-state"),
  ).toBe("working");
  expect(document.querySelector(".chat-pin")).not.toBeNull();
  expect(document.querySelector(".fleet-server-badge")?.textContent).toBe(
    "Remote",
  );
  await act(async () => button?.click());
  expect(firstOpen).not.toHaveBeenCalled();
  expect(nextOpen).toHaveBeenCalledTimes(1);
  await act(async () =>
    document.querySelector<HTMLButtonElement>(".row-actions")?.click(),
  );
  await act(async () => vi.advanceTimersByTimeAsync(0));
  await act(async () => vi.advanceTimersByTimeAsync(200));
  const unread = document.querySelector<HTMLButtonElement>(
    '[role="menuitem"][aria-label="Mark unread"]',
  );
  expect(unread, document.body.innerHTML).not.toBeNull();
  expect(unread?.disabled).toBe(false);
  await act(async () => unread?.click());
  expect(nextUnread).toHaveBeenCalledTimes(1);
  expect(nextOpen).toHaveBeenCalledTimes(1);
  await f.close();
});

it("renders every server unread eligibility and pending-write menu state", async () => {
  const f = fixture();
  const { Menu } = await import("@mantine/core");
  const { MarkUnreadEligibility } = await import("../../generated/api.ts");
  const { UnreadAction } = await import("./UnreadAction.tsx");
  const { actions } = await import("./testFixture.ts");
  const unread = vi.fn();
  for (const eligibility of Object.values(MarkUnreadEligibility)) {
    for (const markingRead of [false, true]) {
      const row = sidebarRowFixture("a", { markUnread: eligibility });
      await act(async () =>
        f.root.render(
          createElement(
            MantineProvider,
            null,
            createElement(
              Menu,
              null,
              createElement(UnreadAction, {
                row,
                disabled: false,
                markingRead,
                actions: { ...actions, unread },
              }),
            ),
          ),
        ),
      );
      const button = document.querySelector<HTMLButtonElement>(
        '[aria-label="Mark unread"]',
      );
      expect(button?.disabled).toBe(
        markingRead || eligibility !== MarkUnreadEligibility.allowed,
      );
      unread.mockClear();
      await act(async () => button?.click());
      expect(unread).toHaveBeenCalledTimes(
        markingRead || eligibility !== MarkUnreadEligibility.allowed ? 0 : 1,
      );
      expect(document.querySelector(".menu-action-help") !== null).toBe(
        markingRead || eligibility !== MarkUnreadEligibility.allowed,
      );
    }
  }
  await f.close();
});

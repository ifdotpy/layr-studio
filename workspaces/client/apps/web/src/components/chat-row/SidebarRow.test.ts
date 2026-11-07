import type { Summary } from "./testFixture.ts";
import { MarkUnreadEligibility } from "../../generated/api.ts";
// @vitest-environment happy-dom
import {
  act,
  createElement,
  Profiler,
  StrictMode,
  type ComponentType,
} from "react";
import { MantineProvider } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { expect, it, vi } from "vitest";
import type { SidebarAgent } from "../sidebar/viewModel.ts";
import SidebarRow from "../SidebarRow.tsx";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";
import { actions, fixture } from "./testFixture.ts";

const commits = vi.hoisted(() => new Map<string, number>());
function counted<P extends object>(View: ComponentType<P>, id: string) {
  return (props: P) =>
    createElement(
      Profiler,
      { id, onRender: () => commits.set(id, (commits.get(id) || 0) + 1) },
      createElement(View, props),
    );
}
vi.mock("./ChatTitle.tsx", async (original) => {
  const module = await original<typeof import("./ChatTitle.tsx")>();
  return { ...module, ChatTitle: counted(module.ChatTitle, "title") };
});
vi.mock("../agents/ChatStatus.tsx", async (original) => {
  const module = await original<typeof import("../agents/ChatStatus.tsx")>();
  return { ...module, default: counted(module.default, "indicator") };
});
vi.mock("../sidebar/LayrCopyProgress.tsx", async (original) => {
  const module =
    await original<typeof import("../sidebar/LayrCopyProgress.tsx")>();
  return { ...module, default: counted(module.default, "progress") };
});
vi.mock("./PinAction.tsx", async (original) => {
  const module = await original<typeof import("./PinAction.tsx")>();
  return { ...module, PinAction: counted(module.PinAction, "pin") };
});
vi.mock("./ArchiveAction.tsx", async (original) => {
  const module = await original<typeof import("./ArchiveAction.tsx")>();
  return { ...module, ArchiveAction: counted(module.ArchiveAction, "archive") };
});
vi.mock("./UnreadAction.tsx", async (original) => {
  const module = await original<typeof import("./UnreadAction.tsx")>();
  return { ...module, UnreadAction: counted(module.UnreadAction, "unread") };
});

it.each([false, true])(
  "isolates the real SidebarRow renderers with its menu open (StrictMode=%s)",
  async (strict) => {
    const f = fixture();
    let parentRenders = 0;
    const Rows = observer(function Rows() {
      parentRenders++;
      return f.model.rows.map((row) =>
        createElement(SidebarRow, {
          key: row.id,
          row,
          selected: false,
          serverAlias: "Server",
          showServer: false,
          renaming: false,
          organizing: false,
          name: "",
          compact: false,
          markingRead: false,
          bindings: {},
          actions,
        }),
      );
    });
    const tree = createElement(MantineProvider, null, createElement(Rows));
    await act(async () =>
      f.root.render(strict ? createElement(StrictMode, null, tree) : tree),
    );
    await act(async () => vi.advanceTimersByTimeAsync(100));
    let row = agentSummaryFixture("a", {
      status: "queued",
      epoch: 1,
      startWait: {
        reason: "workspace_copy",
        epoch: 1,
        progress: { phase: "staging", done: 1, total: 10, unit: "files" },
      },
      markUnread: "allowed",
    });
    const publish = async () =>
      act(async () => {
        f.publish([row]);
        await vi.advanceTimersByTimeAsync(0);
      });
    await publish();
    await act(async () =>
      document.querySelector<HTMLButtonElement>(".row-actions")?.click(),
    );
    await act(async () => vi.advanceTimersByTimeAsync(200));
    expect(
      document.querySelector('[role="menuitem"][aria-label="Mark unread"]'),
    ).not.toBeNull();
    const parentBefore = parentRenders;
    for (const [expected, patch] of [
      [["title"], { name: "Renamed" }],
      [
        ["indicator"],
        {
          operationalStatus: { reason: "working" },
        },
      ],
      [
        ["progress"],
        {
          startWait: {
            reason: "workspace_copy",
            epoch: 1,
            progress: { phase: "staging", done: 2, total: 10, unit: "files" },
          },
        },
      ],
      [["title", "pin"], { pinned: true }],
      [["archive"], { archived: true, inFlight: true }],
      [["unread"], { markUnread: MarkUnreadEligibility["unsupported"] }],
    ] satisfies readonly (readonly [readonly string[], Partial<Summary>])[]) {
      commits.clear();
      row = { ...row, ...patch };
      await publish();
      expect([...commits].sort()).toEqual(expected.map((id) => [id, 1]).sort());
    }
    expect(
      document.querySelector<HTMLButtonElement>('[aria-label="Mark unread"]')
        ?.disabled,
    ).toBe(true);
    commits.clear();
    row = {
      ...row,
      operationalStatus: { ...row.operationalStatus },
      startWait:
        row.startWait?.reason === "workspace_copy"
          ? { ...row.startWait, progress: { ...row.startWait.progress } }
          : row.startWait,
    };
    await publish();
    expect([...commits]).toEqual([]);
    expect(parentRenders).toBe(parentBefore);
    await f.close();
  },
);

it("removing pendingRead restores an unchanged target through the real row", async () => {
  const f = fixture();
  const target = {
    threadId: "thread",
    turnId: "turn",
    revision: 2,
    read: false,
  };
  let pendingRead: SidebarAgent["markRead"] | undefined = { ...target };
  const Rows = observer(function Rows({
    pending,
  }: {
    pending: SidebarAgent["markRead"] | undefined;
  }) {
    return f.model.rows.map((row) =>
      createElement(SidebarRow, {
        key: row.id,
        row,
        pendingRead: pending,
        selected: false,
        serverAlias: "Server",
        showServer: false,
        renaming: false,
        organizing: false,
        name: "",
        compact: false,
        markingRead: false,
        bindings: {},
        actions,
      }),
    );
  });
  const render = async () =>
    act(async () =>
      f.root.render(
        createElement(
          MantineProvider,
          null,
          createElement(Rows, { pending: pendingRead }),
        ),
      ),
    );
  await render();
  await act(async () => vi.advanceTimersByTimeAsync(100));
  await act(async () => {
    f.publish([
      agentSummaryFixture("a", {
        operationalStatus: { reason: "unread_result" },
        markRead: target,
      }),
    ]);
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(document.querySelector('[data-chat-status="unread"]')).toBeNull();
  pendingRead = undefined;
  await render();
  expect(document.querySelector('[data-chat-status="unread"]')).not.toBeNull();
  expect(document.querySelector('[data-studio-state="unread"]')).not.toBeNull();
  expect(
    document.querySelector(".chat-row")?.getAttribute("aria-description"),
  ).toContain("Unread result");
  await f.close();
});

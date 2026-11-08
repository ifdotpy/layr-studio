import { createStateMirror } from "../../generated/observableResources.ts";
import { SidebarOrderOwner } from "../useSidebarOrder.ts";
import { localSidebarServices } from "../sidebar/services.tsx";
import { rowArea } from "./actions.fixture.ts";
import { SidebarRowActionsProvider } from "./actions.ts";
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
import ProjectStateCounts from "../sidebar/ProjectStateCounts.tsx";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";
import { actions, fixture, makeRow } from "./testFixture.ts";
import { observable, runInAction } from "mobx";
import { type SidebarRowActions } from "./actions.ts";

it("the count leaf observes status without observing chat names", async () => {
  const f = fixture();
  const mirror = createStateMirror();
  const apply = (name: string, reason: "working" | "failed") =>
    mirror.apply([
      {
        collection: "agentSummary",
        id: "a",
        value: agentSummaryFixture("a", {
          name,
          operationalStatus: { reason },
        }),
      },
    ]);
  apply("First", "working");
  const agent = mirror.current.agents.get("a");
  if (!agent) throw new Error("Missing applied model");
  const rows = [makeRow(agent)];
  const rendered = vi.fn();
  await act(async () =>
    f.root.render(
      createElement(
        Profiler,
        { id: "counts", onRender: rendered },
        createElement(ProjectStateCounts, { rows, archived: false }),
      ),
    ),
  );
  rendered.mockClear();
  await act(async () => apply("Renamed", "working"));
  expect(rendered).not.toHaveBeenCalled();
  await act(async () => apply("Renamed", "failed"));
  expect(rendered).toHaveBeenCalledTimes(1);
  expect(
    document.querySelector(".project-state-counts")?.getAttribute("aria-label"),
  ).toBe("1 failed");
  await f.close();
});

it.each([false, true])(
  "isolates sibling rows and priority copies from UI changes (priority=%s)",
  async (priority) => {
    const f = fixture();
    const ui = observable({
      services: localSidebarServices,
      aliases: observable.map<string, string>(),
      get sorting() {
        return sorting;
      },
      offline: observable.set<string>(),
      selection: { selectedId: null as string | null },
      read: {
        pendingReadTargets: new Map<
          string,
          NonNullable<SidebarAgent["markRead"]>
        >(),
        marking: new Set<string>(),
      },
      renaming: null as string | null,
      renameInPriority: false,
      name: "",
      organizing: null as string | null,
      visibility: { isCompact: false },
      overrides: observable.map<
        string,
        { pinned?: boolean; archived?: boolean }
      >(),
    });
    const sorting = new SidebarOrderOwner({
      stateDir: "/row-isolation",
      runtime: undefined,
      services: localSidebarServices,
    });
    const area: SidebarRowActions = {
      ui,
      commands: () => actions,
      disabled: (row) => ui.offline.has(row.id),
      server: (row) => ({
        alias: ui.aliases.get(row.id) || "Server",
        show: true,
        offline: ui.offline.has(row.id),
      }),
      bindings: (row) =>
        sorting.bindings(
          row.groupKey,
          row.id,
          f.model.rows.map((item) => item.id),
        ),
    };
    const renders = new Map<string, number>();
    const Rows = observer(() => {
      return f.model.rows.flatMap((row) =>
        [false, true].map((copy) =>
          createElement(
            Profiler,
            {
              key: `${row.id}:${copy}`,
              id: `${row.id}:${copy}`,
              onRender: (id) => renders.set(id, (renders.get(id) || 0) + 1),
            },
            createElement(SidebarRow, { row, priority: copy }),
          ),
        ),
      );
    });
    await act(async () =>
      f.root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            SidebarRowActionsProvider,
            { value: area },
            createElement(Rows),
          ),
        ),
      ),
    );
    await act(async () => {
      f.publish([agentSummaryFixture("a"), agentSummaryFixture("b")]);
      await vi.advanceTimersByTimeAsync(100);
    });
    renders.clear();
    await act(async () =>
      runInAction(() => {
        ui.renaming = "a";
        ui.renameInPriority = priority;
      }),
    );
    expect(renders.get(`a:${priority}`)).toBeGreaterThan(0);
    expect(renders.get(`a:${!priority}`) || 0).toBe(0);
    expect(renders.get("b:false") || 0).toBe(0);
    expect(renders.get("b:true") || 0).toBe(0);
    renders.clear();
    await act(async () =>
      runInAction(() => {
        ui.name = "Typed draft";
      }),
    );
    expect(renders.get(`a:${priority}`)).toBeGreaterThan(0);
    expect(renders.get(`a:${!priority}`) || 0).toBe(0);
    expect(renders.get("b:false") || 0).toBe(0);
    expect(renders.get("b:true") || 0).toBe(0);
    await act(async () =>
      runInAction(() => {
        ui.renaming = null;
      }),
    );
    await act(async () =>
      document.querySelector<HTMLButtonElement>(".row-actions")?.click(),
    );
    await act(async () => vi.advanceTimersByTimeAsync(200));
    renders.clear();
    await act(async () =>
      runInAction(() => {
        ui.read.marking.add("a");
      }),
    );
    expect(renders.get("a:false")).toBeGreaterThan(0);
    expect(renders.get("b:false") || 0).toBe(0);
    expect(renders.get("b:true") || 0).toBe(0);
    for (const update of [
      () =>
        ui.read.pendingReadTargets.set("a", {
          threadId: "thread",
          turnId: "turn",
          revision: 2,
          read: false,
        }),
      () => ui.read.pendingReadTargets.delete("a"),
      () => ui.offline.add("a"),
      () => ui.aliases.set("a", "Changed server"),
      () => {
        sorting.setDestination("convert:a");
      },
      () => {
        sorting.setTarget({
          group: f.model.rows[0]!.groupKey,
          id: "a",
          after: true,
        });
      },
    ]) {
      renders.clear();
      await act(async () =>
        runInAction(() => {
          update();
        }),
      );
      expect(renders.get("a:false")).toBeGreaterThan(0);
      expect(renders.get("a:true")).toBeGreaterThan(0);
      expect(renders.get("b:false") || 0).toBe(0);
      expect(renders.get("b:true") || 0).toBe(0);
    }
    expect(
      document.querySelector<HTMLElement>('[data-chat="a"]')?.dataset
        .folderDrop,
    ).toBe("true");
    expect(
      document.querySelector<HTMLElement>('[data-chat="a"]')?.dataset.dropEdge,
    ).toBe("after");
    await f.close();
  },
);

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
        createElement(
          SidebarRowActionsProvider,
          {
            value: rowArea(() => actions, {}),
            key: row.id,
          },
          createElement(SidebarRow, { key: row.id, row }),
        ),
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
      createElement(
        SidebarRowActionsProvider,
        {
          value: rowArea(() => actions, { pendingRead: pending }),
          key: row.id,
        },
        createElement(SidebarRow, { key: row.id, row }),
      ),
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

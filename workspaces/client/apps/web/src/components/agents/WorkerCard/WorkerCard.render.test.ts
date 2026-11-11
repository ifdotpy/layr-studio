import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import { AgentSource, AgentStatus } from "../../../generated/api.ts";
// @vitest-environment happy-dom
import { act, createElement, Profiler, type ComponentType } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import TeamPanel from "../support/TeamFixture.tsx";
import { TeamProvider, useTeamArea } from "../support/TeamPanelContext.tsx";
import { TeamPresence } from "../TeamPresence/TeamPresence.tsx";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import type { Agent } from "../../../types.ts";
import { waitingChildren } from "../PhaseWaitDetails/waitDetails.ts";
import { autorun, computed } from "mobx";

const h = vi.hoisted(() => ({
  commits: new Map<string, number>(),
  rows: 0,
  sync: { subscribeStateProjection: () => () => {} },
}));
function counted<P extends object>(View: ComponentType<P>, name: string) {
  return (props: P) =>
    createElement(
      Profiler,
      {
        id: name,
        onRender: () => h.commits.set(name, (h.commits.get(name) || 0) + 1),
      },
      createElement(View, props),
    );
}
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerSync: () => h.sync,
  }),
);
vi.mock("../../conversation/TokenRate/TokenRate.tsx", () => ({
  default: () => null,
}));
vi.mock("../support/WorkerModelPicker.tsx", () => ({
  useWorkerModels: () => ({ models: [] }),
}));
vi.mock("../../accounts/Accounts/Accounts.tsx", async (original) => ({
  ...(await original<typeof import("../../accounts/Accounts/Accounts.tsx")>()),
  useAccounts: () => ({ data: { accounts: [] } }),
}));
vi.mock("../../../perf/promptComposerRenderProbe.ts", () => ({
  reportPromptComposerRender: (component: string) => {
    if (component === "team-row") h.rows++;
  },
}));
vi.mock("../worker/WorkerActivity/WorkerActivity.tsx", async (original) => {
  const m =
    await original<
      typeof import("../worker/WorkerActivity/WorkerActivity.tsx")
    >();
  return { ...m, WorkerActivity: counted(m.WorkerActivity, "activity") };
});
vi.mock("../worker/WorkerMetadata/WorkerMetadata.tsx", async (original) => {
  const m =
    await original<
      typeof import("../worker/WorkerMetadata/WorkerMetadata.tsx")
    >();
  return { ...m, WorkerMetadata: counted(m.WorkerMetadata, "metadata") };
});
vi.mock("../../sidebar/chat-row/ChatTitle/ChatTitle.tsx", async (original) => {
  const m =
    await original<
      typeof import("../../sidebar/chat-row/ChatTitle/ChatTitle.tsx")
    >();
  return { ...m, ChatTitle: counted(m.ChatTitle, "title") };
});
vi.mock("../ChatStatus/ChatStatus.tsx", async (original) => {
  const m = await original<typeof import("../ChatStatus/ChatStatus.tsx")>();
  return { ...m, default: counted(m.default, "status") };
});

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: () => true,
  }));
  h.commits.clear();
  h.rows = 0;
});

it("keeps expandable wait names on the same generated direct-child models", () => {
  const wait = { agents: 1, commands: 0, monitors: 0, inputs: 0, event: null };
  const f = observableFixture([
    { id: "lead", source: AgentSource["managed"], isLead: true },
    {
      id: "middle",
      source: AgentSource["managed"],
      parentId: "lead",
      status: AgentStatus["waiting"],
      operationalStatus: { reason: "waiting", wait },
    },
    {
      id: "child",
      source: AgentSource["managed"],
      parentId: "middle",
      status: AgentStatus["waiting"],
      hasBackgroundWork: true,
    },
    {
      id: "sibling",
      source: AgentSource["managed"],
      parentId: "lead",
      status: AgentStatus["running"],
      inFlight: true,
    },
    {
      id: "unmanaged",
      source: AgentSource["registered"],
      parentId: "lead",
      status: AgentStatus["running"],
      inFlight: true,
    },
  ]);
  const children = computed(() =>
    waitingChildren(f.mirror.current.agents.values(), "lead"),
  );
  const branch = computed(() =>
    waitingChildren(f.mirror.current.agents.values(), "middle"),
  );
  const stop = autorun(() => {
    void children.get();
    void branch.get();
  });
  try {
    expect(children.get().map((agent) => agent.id)).toEqual([
      "middle",
      "sibling",
    ]);
    expect(children.get()[0]).toBe(f.mirror.current.agents.get("middle"));
    expect(children.get()[0]?.parentId).toBe("lead");
    expect(branch.get().map((agent) => agent.id)).toEqual(["child"]);
    expect(branch.get()[0]?.parentId).toBe("middle");
  } finally {
    stop();
    f.store.dispose();
  }
});
afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

it("dereferences worker fields in independent leaves and keeps equal replay silent", async () => {
  const lead: Agent = {
    id: "lead",
    source: AgentSource["managed"],
    isLead: true,
  };
  const worker: Agent = {
    id: "worker",
    source: AgentSource["managed"],
    rootId: "lead",
    parentId: "lead",
    name: "Worker",
    status: AgentStatus["running"],
    operationalStatus: { reason: "working" },
    inFlight: true,
    effort: "medium",
    activity: { phase: "thinking" },
    overview: { task: "Review" },
  };
  const f = observableFixture([lead, worker]);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          f.Provider,
          {},
          createElement(
            MantineProvider,
            {},
            createElement(TeamPanel, { leadId: "lead", opened: "lead" }),
          ),
        ),
      ),
    );
    const rows = h.rows;
    const initial = new Map(h.commits);
    const summary = f.mirror.current.agents.get("worker")!;
    const renamed = { ...summary, name: "Renamed" };
    await act(async () =>
      f.apply([
        {
          collection: "agentSummary",
          id: "lead",
          value: f.mirror.current.agents.get("lead")!,
        },
        { collection: "agentSummary", id: "worker", value: renamed },
      ]),
    );
    expect(host.textContent).toContain("Renamed");
    expect(h.rows).toBe(rows);
    expect(h.commits.get("title")).toBe(initial.get("title")! + 1);
    for (const leaf of ["activity", "metadata", "status"])
      expect(h.commits.get(leaf)).toBe(initial.get(leaf));

    const beforeDetail = new Map(h.commits);
    await act(async () =>
      f.details.get("worker")!.apply({ ...worker, effort: "high" }),
    );
    expect(host.querySelector(".worker-effort")?.textContent).toBe("High");
    expect(h.rows).toBe(rows);
    expect(h.commits.get("metadata")).toBe(beforeDetail.get("metadata")! + 1);
    for (const leaf of ["activity", "title", "status"])
      expect(h.commits.get(leaf)).toBe(beforeDetail.get(leaf));

    const beforeEqual = new Map(h.commits);
    await act(async () =>
      f.details.get("worker")!.apply({ ...worker, effort: "high" }),
    );
    expect(h.commits).toEqual(beforeEqual);
    expect(h.rows).toBe(rows);
  } finally {
    await act(async () => root.unmount());
    f.store.dispose();
  }
});

it("reveals the team from generated membership without rerendering its area root", async () => {
  const fixture = observableFixture();
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  let areaRenders = 0;
  const Gate = counted(TeamPresence, "presence");
  const noop = () => {};
  function Area() {
    areaRenders++;
    const area = useTeamArea("lead", "", "all");
    return createElement(
      TeamProvider,
      {
        value: {
          ...area,
          open: noop,
          previewResult: noop,
          remove: noop,
          opened: null,
          workerQuery: "",
          workerFilter: "all",
          completedOpen: false,
          setWorkerQuery: noop,
          setWorkerFilter: noop,
          setCompletedOpen: noop,
          setTeamOpen: noop,
          setWideTeamOpen: noop,
        },
      },
      createElement(Gate, { children: "Team" }),
    );
  }
  const lead = agentSummaryFixture("lead");
  const worker = agentSummaryFixture("worker", {
    isLead: false,
    rootId: "lead",
    parentId: "lead",
  });
  const apply = () =>
    fixture.apply([
      { collection: "agentSummary", id: lead.id, value: lead },
      { collection: "agentSummary", id: worker.id, value: worker },
    ]);
  try {
    await act(async () =>
      root.render(createElement(fixture.Provider, {}, createElement(Area))),
    );
    expect(host.textContent).toBe("");
    const before = areaRenders;
    await act(async () => apply());
    expect(host.textContent).toBe("Team");
    expect(areaRenders).toBe(before);
    const commits = h.commits.get("presence");
    worker.operationalStatus = { reason: "working" };
    await act(async () => apply());
    expect(h.commits.get("presence")).toBe(commits);
    expect(areaRenders).toBe(before);
  } finally {
    await act(async () => root.unmount());
    fixture.store.dispose();
    host.remove();
  }
});

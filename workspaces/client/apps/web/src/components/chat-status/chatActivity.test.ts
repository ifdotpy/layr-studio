import { autorun, computed } from "mobx";
import { createProcessesMirror } from "../../generated/observableResources.ts";
import { unreadResult } from "./readReceiptFixture.ts";
import {
  waitLabel,
  formatOperationalStatus,
} from "../../status/operationalStatus.ts";
import { chatActivities as mapActivities } from "./chatActivity.ts";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";
import type { SidebarAgent } from "../sidebar/viewModel.ts";
import { CHAT_INDICATOR_URGENCY } from "../../status/operationalStatus.ts";
import {
  AgentSource,
  AgentStatus,
  TaskKind,
  TaskStatus,
} from "../../generated/api";
import { backgroundDisplay, required } from "../componentTestFixtures";

import type { ProcessList } from "../../generated/observableResources.ts";
import type { Agent, Snapshot } from "../../types";

import { componentSnapshot } from "../componentTestFixtures";
import assert from "node:assert/strict";
import { backgroundActivities } from "./chatActivity.ts";
import { it } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFileSync } from "node:fs";
import { PROVIDER_KINDS } from "../../generated/providerKind.ts";
import ChatStatus from "../agents/ChatStatus";
function chatActivities(data: Snapshot) {
  const entries: ProcessList["activities"] = [
    ...data.runtime.tasks.map((record) => ({
      source: "task" as const,
      value: {
        ...record,
        agent: record.agent || "",
        name: record.name || "",
        created: record.created ?? 0,
        turnId: record.turnId || "",
        kind:
          record.kind ||
          (record.command ? ("command" as const) : ("tool" as const)),
        status: record.status || ("running" as const),
      },
    })),
    ...data.runtime.monitors.map((record) => ({
      source: "process" as const,
      value: {
        ...record,
        agent: record.agent || "",
        created: record.created ?? 0,
        name: record.name || "",
        status: record.status || ("running" as const),
        key: record.id,
        generation: "generation",
        accountKey: "default",
        epoch: 1,
        source: "monitor" as const,
        display:
          record.display ??
          backgroundDisplay({ kind: "monitor", kindLabel: "monitor" }),
        bytes: 0,
        cancelRequested: false,
        outputTruncated: false,
        outputPatterns: [],
        tail: "",
      },
    })),
  ];
  return mapActivities(data.threads, entries);
}
it("activity CSS selectors consume every generated provider identity", () => {
  const css = readFileSync(
    new URL("../agents/provider-activity.css", import.meta.url),
    "utf8",
  );
  const identities = [
    ...css.matchAll(/\[data-agent-provider="([^"]+)"\]/g),
  ].map((match) => match[1]);
  assert.deepEqual(
    identities.sort(),
    [...PROVIDER_KINDS].sort(),
    "generated/providerKind.ts owns provider-activity.css identities",
  );
});
it("chat status preserves unknown and missing identities without a provider default", () => {
  for (const [provider, label] of [
    ["codex", "Codex"],
    ["claude", "Claude"],
    ["Future Provider", "Future Provider"],
    ["CODEX", "CODEX"],
    [" claude ", " claude "],
    ["", ""],
    [undefined, ""],
  ]) {
    const markup = renderToStaticMarkup(
      createElement(ChatStatus, {
        agent: {
          ...agentSummaryFixture("test"),
          operationalStatus: { reason: "working" },
          provider,
          model: "custom-model",
        } as SidebarAgent,
      }),
    );
    assert.ok(markup.includes(`${label} · custom-model`), markup);
    if (provider !== undefined)
      assert.ok(markup.includes(`data-agent-provider="${provider}"`), markup);
    else assert.ok(!markup.includes('data-agent-provider="codex"'), markup);
  }
});
it("preserves visible activity and owned wait rules beside the server reason formatter", () => {
  const lead: Agent = {
    id: "lead",
    rootId: "lead",
    name: "Lead",
    source: AgentSource.managed,
    isLead: true,
    status: AgentStatus.completed,
    autoWake: true,
    threadId: "thread",
    lastCompletedTurn: "turn",
    lastCompletedTurnStatus: "completed",
    epoch: 2,
  };
  const child: Agent = {
    ...lead,
    id: "child",
    rootId: "lead",
    parentId: "lead",
    isLead: false,
  };
  const snapshot = (
    agents: Agent[] = [lead],
    runtime: Parameters<typeof componentSnapshot>[1] = {},
  ) => componentSnapshot(agents, runtime);
  for (const readState of [
    { threadId: "old", turnId: "turn", read: true },
    { threadId: "thread", turnId: "old", read: true },
    { threadId: "thread", turnId: "turn", read: false },
  ] as const)
    // Legacy read-state fixtures omit revision.
    assert(
      unreadResult({
        ...lead,
        readState,
      } as Agent),
    );
  const pausedChild: Agent = {
    ...child,
    status: AgentStatus.paused,
    autoWake: false,
    inFlight: false,
  };
  const command = {
    id: "long-command",
    agent: child.id,
    status: "running" as const,
    command: "find /Users/igor",
    created: 10,
    kind: TaskKind.command,
    display: backgroundDisplay(),
  };
  const activeSnapshot = snapshot([lead, pausedChild], { tasks: [command] });
  const reasons = chatActivities(activeSnapshot);
  assert.equal(required(required(reasons.get("lead"))[0]).id, command.id);
  assert.equal(
    required(required(reasons.get("lead"))[0]).agentName,
    child.name,
  );
  assert.equal(
    required(required(reasons.get("lead"))[0]).command,
    command.command,
  );
  assert.equal(required(required(reasons.get("child"))[0]).created, 10);
  assert.equal(
    chatActivities(
      snapshot([lead, pausedChild], {
        tasks: [{ ...command, status: TaskStatus.completed }],
      }),
    ).size,
    0,
  );
  const activeChild: Agent = {
    ...child,
    status: AgentStatus.running,
    inFlight: true,
  };
  assert.equal(
    required(
      chatActivities(snapshot([lead, activeChild], { tasks: [command] })).get(
        "lead",
      ),
    ).length,
    1,
  );
  assert.equal(
    required(
      required(chatActivities(snapshot([lead, activeChild])).get("lead"))[0],
    ).kind,
    "agent",
  );
  const otherRoot: Agent = { ...lead, id: "other-root", rootId: "other-root" };
  assert.equal(
    chatActivities(
      snapshot([lead, pausedChild, otherRoot], { tasks: [command] }),
    ).has("other-root"),
    false,
  );
  // The bar above the chat shows only monitors and commands outside their turn.
  const turnChild: Agent = {
    ...child,
    status: AgentStatus.running,
    inFlight: true,
    turnId: "t2",
  };
  const strip = (
    tasks: Snapshot["runtime"]["tasks"],
    monitors: Snapshot["runtime"]["monitors"] = [],
    agents: Agent[] = [lead, turnChild],
  ) =>
    backgroundActivities(
      required(
        chatActivities(snapshot(agents, { tasks, monitors })).get("lead"),
      ) || [],
    ).map((activity) => activity.id);
  assert.deepEqual(strip([{ ...command, turnId: "t2" }]), []);
  assert.deepEqual(strip([{ ...command, turnId: "t1" }]), [command.id]);
  assert.deepEqual(strip([command], [], [lead, pausedChild]), [command.id]);
  assert.deepEqual(
    strip(
      [],
      [
        {
          id: "watch",
          agent: child.id,
          created: 1,
          status: TaskStatus.running,
          display: backgroundDisplay({ kind: "monitor", kindLabel: "monitor" }),
        },
      ],
    ),
    ["watch"],
  );
  assert.deepEqual(strip([]), []);
  assert.equal(
    required(
      required(chatActivities(snapshot([lead, turnChild])).get("lead"))[0],
    ).kind,
    "agent",
  );
  console.log(
    "PASS visible activity reasons match spinner, including live commands from stopped workers and scope boundaries",
  );
});
it("preserves every former wait-label assertion from explicit server counts", () => {
  const label = (
    agents = 0,
    commands = 0,
    monitors = 0,
    inputs = 0,
    event: string | null = null,
  ) => waitLabel({ agents, commands, monitors, inputs, event });
  assert.equal(
    formatOperationalStatus({ reason: "turn_ended" }).label,
    "Turn ended. Send a message to continue.",
  );
  assert.equal(label(1), "Waiting for 1 agent");
  assert.equal(label(0, 1), "Waiting for 1 command");
  assert.equal(label(0, 0, 1), "Waiting for 1 monitor");
  assert.equal(
    label(0, 0, 0, 0, "build-ready"),
    "Waiting for event build-ready",
  );
  assert.equal(label(0, 0, 0, 1), "Waiting for 1 input");
  assert.equal(label(2), "Waiting for 2 agents");
  assert.equal(label(2, 0, 1), "Waiting for 2 agents and 1 monitor");
  assert.equal(label(2, 1, 1), "Waiting for 2 agents, 1 command and 1 monitor");
  assert.equal(
    label(0, 2, 2, 2, "ready"),
    "Waiting for 2 commands, 2 monitors, event ready and 2 inputs",
  );
});
const makeAgent = (overrides: Partial<Agent> = {}): Agent => ({
  id: "lead",
  rootId: "lead",
  name: "Lead",
  source: AgentSource.managed,
  isLead: true,
  status: AgentStatus.completed,
  autoWake: true,
  epoch: 2,
  threadId: "thread",
  lastCompletedTurn: "turn",
  lastCompletedTurnStatus: "completed",
  ...overrides,
});
const makeSnapshot = componentSnapshot;
it("maps live runtime records to sorted caller-visible activities", () => {
  const lead = makeAgent();
  const child = makeAgent({
    id: "child",
    rootId: "lead",
    parentId: "lead",
    isLead: false,
    name: "Worker",
    status: "running",
    inFlight: true,
    turnId: "current",
  });
  const snapshot = makeSnapshot([lead, child], {
    tasks: [
      { id: "z", agent: "child", status: TaskStatus.running, created: 10 },
      {
        id: "command",
        agent: "child",
        kind: TaskKind.command,
        command: "git status",
        status: TaskStatus.approval,
        created: 2,
        turnId: "old",
      },
      {
        id: "tool",
        agent: "child",
        query: "search query",
        type: "search",
        status: TaskStatus.starting,
        created: 2,
        turnId: "current",
      },
      {
        id: "ignored-status",
        agent: "child",
        status: TaskStatus.completed,
        command: "done",
      },
      { id: "unknown-agent", agent: "missing", status: TaskStatus.running },
    ],
    monitors: [
      {
        id: "monitor",
        agent: "child",
        command: "watch",
        name: "watcher",
        status: TaskStatus.running,
        created: 2,
      },
      {
        id: "monitor-no-command",
        agent: "child",
        name: "watcher without a command",
        status: TaskStatus.running,
        created: 2,
      },
    ],
  });
  const activities = chatActivities(snapshot);
  assert.deepEqual(
    required(activities.get("lead")).map(({ id }) => id),
    ["command", "monitor", "monitor-no-command", "tool", "z"],
    "activities are shared with the root chat and sort by time, then ID",
  );
  assert.deepEqual(required(required(activities.get("lead"))[0]), {
    id: "command",
    kind: "task",
    agentId: "child",
    agentName: "Worker",
    label: "Background command",
    command: "git status",
    created: 2,
    status: "approval",
    background: true,
    commandTask: true,
  });
  assert.deepEqual(required(required(activities.get("lead"))[1]), {
    id: "monitor",
    kind: "monitor",
    agentId: "child",
    agentName: "Worker",
    label: "Background command",
    command: "watch",
    created: 2,
    status: "running",
    background: true,
    commandTask: false,
  });
  assert.equal(
    required(required(activities.get("lead"))[2]).label,
    "Background command",
  );
  assert.equal(
    required(required(activities.get("lead"))[2]).command,
    "watcher without a command",
  );
  assert.equal(
    required(required(activities.get("lead"))[3]).label,
    "Tool call",
  );
  assert.equal(
    required(required(activities.get("lead"))[3]).command,
    "search query",
  );
  assert.equal(required(required(activities.get("lead"))[3]).background, false);
  assert.equal(
    required(required(activities.get("lead"))[3]).commandTask,
    false,
  );
  assert.equal(required(activities.get("child")).length, 5);
  assert.deepEqual(
    backgroundActivities(required(activities.get("lead"))).map(({ id }) => id),
    ["command", "monitor", "monitor-no-command"],
  );
  const selfActivity = chatActivities(
    makeSnapshot([lead], {
      tasks: [
        {
          id: "self",
          agent: "lead",
          status: TaskStatus.running,
          command: "pwd",
        },
      ],
    }),
  );
  assert.deepEqual(
    required(selfActivity.get("lead")).map(({ id }) => id),
    ["self"],
  );
  const unknownRootActivity = chatActivities(
    makeSnapshot(
      [makeAgent({ id: "orphan", rootId: "missing-root", inFlight: true })],
      {
        tasks: [
          { id: "orphan-task", agent: "orphan", status: TaskStatus.running },
        ],
      },
    ),
  );
  assert.equal(unknownRootActivity.has("missing-root"), false);
});
it("uses the runtime command, turn, epoch and fallback rules for activities", () => {
  const lead = makeAgent({ inFlight: true, turnId: "turn-3" });
  const noEpoch = makeAgent({
    id: "no-epoch",
    rootId: "lead",
    epoch: null,
    name: "No epoch",
    status: "paused",
    inFlight: false,
  });
  const child = makeAgent({
    id: "child",
    rootId: "lead",
    parentId: "lead",
    name: "Worker",
    status: "running",
    inFlight: true,
    turnId: "turn-3",
  });
  const activities = chatActivities(
    makeSnapshot([lead, noEpoch, child], {
      tasks: [
        {
          id: "command-kind-only",
          agent: "lead",
          kind: TaskKind.command,
          status: TaskStatus.running,
          type: "exec",
        },
        {
          id: "command-without-kind",
          agent: "lead",
          status: TaskStatus.running,
          command: "git diff",
        },
        {
          id: "same-turn",
          agent: "lead",
          status: TaskStatus.running,
          command: "pwd",
          turnId: "turn-3",
        },
        {
          id: "different-turn",
          agent: "lead",
          status: TaskStatus.running,
          command: "ls",
          turnId: "turn-2",
        },
        {
          id: "unscoped-turn",
          agent: "lead",
          status: TaskStatus.running,
          command: "cat",
        },
        {
          id: "agent-without-epoch",
          agent: "no-epoch",
          status: TaskStatus.running,
          name: "named task",
        },
        {
          id: "tool-by-kind",
          agent: "child",
          kind: TaskKind.tool,
          status: TaskStatus.running,
          type: "exec",
        },
      ],
    }),
  );
  assert.deepEqual(
    required(activities.get("lead")).map(({ id }) => id),
    [
      "agent-without-epoch",
      "command-kind-only",
      "command-without-kind",
      "different-turn",
      "same-turn",
      "tool-by-kind",
      "unscoped-turn",
    ],
  );
  assert.deepEqual(
    required(activities.get("lead"))
      .map(({ id }) => id)
      .filter((id) =>
        backgroundActivities(required(activities.get("lead"))).some(
          (entry) => entry.id === id,
        ),
      ),
    ["different-turn"],
  );
  assert.equal(
    required(activities.get("lead")).find(({ id }) => id === "same-turn")
      ?.label,
    "Command",
  );
  assert.equal(
    required(activities.get("lead")).find(
      ({ id }) => id === "command-without-kind",
    )?.commandTask,
    true,
  );
  assert.equal(
    required(activities.get("lead")).find(
      ({ id }) => id === "command-kind-only",
    )?.commandTask,
    true,
  );
  assert.equal(
    required(activities.get("lead")).find(({ id }) => id === "tool-by-kind")
      ?.command,
    "tool",
  );
  assert.equal(
    required(required(activities.get("no-epoch"))[0]).command,
    "named task",
  );
  assert.equal(
    required(required(activities.get("no-epoch"))[0]).label,
    "Tool call",
  );
  const fallback = (agent: Agent) =>
    chatActivities(makeSnapshot([agent])).get(agent.id)?.[0];
  for (const [status, label] of [
    ["queued", "Waiting to start"],
    ["starting", "Starting"],
    ["running", "Working"],
  ] as const) {
    assert.equal(
      fallback(makeAgent({ id: status, status, inFlight: false }))?.label,
      label,
    );
  }
  assert.equal(
    fallback(makeAgent({ id: "in-flight", status: "paused", inFlight: true }))
      ?.label,
    "Working",
  );
  for (const agent of [
    makeAgent({ id: "auto-wake-off", status: "running", autoWake: false }),
    makeAgent({ id: "completed", status: "completed", inFlight: false }),
    makeAgent({ id: "idle", status: "idle", inFlight: false }),
    makeAgent({
      id: "not-managed",
      source: "local" as NonNullable<Agent["source"]>,
      status: "running",
      inFlight: true,
    }),
  ] as const)
    assert.equal(fallback(agent), undefined);
  const concreteWins = chatActivities(
    makeSnapshot(
      [makeAgent({ id: "concrete", status: "running", inFlight: true })],
      {
        tasks: [
          {
            id: "actual",
            agent: "concrete",
            status: TaskStatus.running,
            command: "make",
          },
        ],
      },
    ),
  );
  assert.deepEqual(
    required(concreteWins.get("concrete")).map(({ id }) => id),
    ["actual"],
  );
});
it("builds live wait labels from checked server reasons", () => {
  assert.equal(
    waitLabel({
      agents: 1,
      commands: 1,
      monitors: 1,
      event: "release-ready",
      inputs: 1,
    }),
    "Waiting for 1 agent, 1 command, 1 monitor, event release-ready and 1 input",
  );
  assert.equal(
    waitLabel({ agents: 0, commands: 0, monitors: 0, event: null, inputs: 1 }),
    "Waiting for 1 input",
  );
  assert.equal(
    waitLabel({
      agents: 1,
      commands: 0,
      monitors: 0,
      event: "release-ready",
      inputs: 0,
    }),
    "Waiting for 1 agent and event release-ready",
  );
});
it("keeps local completion receipt identity checks", () => {
  const agent = makeAgent();
  assert.equal(unreadResult(agent), true);
  assert.equal(unreadResult({ ...agent, threadId: null }), false);
  assert.equal(unreadResult({ ...agent, lastCompletedTurn: null }), false);
  assert.equal(
    unreadResult({ ...agent, lastCompletedTurnStatus: "failed" }),
    false,
  );
  assert.equal(
    unreadResult(agent, {
      threadId: "thread",
      turnId: "turn",
      read: true,
      revision: 2,
    }),
    false,
  );
  assert.equal(
    unreadResult(agent, {
      threadId: "thread",
      turnId: "other",
      read: true,
      revision: 2,
    }),
    true,
  );
  assert.equal(unreadResult(agent, null), true);
  assert.equal(
    unreadResult({
      ...agent,
      readState: {
        threadId: "thread",
        turnId: "turn",
        read: true,
        revision: 2,
      },
    }),
    false,
    "omitting the read-state argument uses the agent's current receipt",
  );
});
it("orders every indicator kind once by operator urgency", () => {
  assert.deepEqual(CHAT_INDICATOR_URGENCY, [
    "answer",
    "error",
    "working",
    "unread",
    "paused",
    "none",
    "loading",
  ]);
});

it("keeps generated activity membership and derived activities stable for tail-only updates", () => {
  const agent = makeAgent({ inFlight: true, turnId: "current" });
  const listing: ProcessList = {
    processes: [],
    activities: [
      {
        source: "task",
        value: {
          id: "tool",
          agent: agent.id,
          created: 1,
          name: "Search",
          kind: TaskKind.tool,
          status: TaskStatus.running,
          turnId: "current",
          tail: "before",
          display: backgroundDisplay({ kind: "tool" }),
        },
      },
    ],
  };
  const mirror = createProcessesMirror(listing);
  const membership = mirror.current.activities;
  const entry = required(membership[0]);
  const view = computed(() =>
    mapActivities([agent], mirror.current.activities),
  );
  let updates = 0;
  const stop = autorun(() => {
    void view.get();
    updates++;
  });
  try {
    const before = view.get();
    mirror.apply({
      ...listing,
      activities: listing.activities.map((entry) =>
        entry.source === "task"
          ? { ...entry, value: { ...entry.value, tail: "after" } }
          : { ...entry, value: { ...entry.value, tail: "after" } },
      ),
    });
    assert.equal(mirror.current.activities, membership);
    assert.equal(mirror.current.activities[0], entry);
    assert.equal(entry.value.tail, "after");
    assert.equal(view.get(), before);
    assert.equal(updates, 1);
  } finally {
    stop();
  }
});

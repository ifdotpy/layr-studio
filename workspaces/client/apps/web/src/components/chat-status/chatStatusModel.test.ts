import {
  AgentSource,
  AgentStatus,
  TaskKind,
  TaskStatus,
} from "../../generated/api";
import { backgroundDisplay, required } from "../componentTestFixtures";
import { simpleRequest } from "../../approvalFixtures";
import type { Agent, Snapshot } from "../../types";
import type { ChatActivity } from "./chatStatusModel";
import { componentSnapshot } from "../componentTestFixtures";
import assert from "node:assert/strict";
import {
  CHAT_INDICATOR_URGENCY,
  unreadResult,
  chatActivities,
  backgroundActivities,
  chatWaitState,
} from "./chatStatusModel.ts";
import { it } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFileSync } from "node:fs";
import { PROVIDER_KINDS } from "../../generated/providerKind.ts";
import ChatStatus from "../agents/ChatStatus";
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
        status: { kind: "working", label: "Working" },
        provider,
        model: "custom-model",
      }),
    );
    assert.ok(markup.includes(`${label} · custom-model`), markup);
    if (provider !== undefined)
      assert.ok(markup.includes(`data-agent-provider="${provider}"`), markup);
    else assert.ok(!markup.includes('data-agent-provider="codex"'), markup);
  }
});
// Partial activity fixtures omit display-only agentName, label and status.
type ActivityFixture = Omit<ChatActivity, "agentName" | "label" | "status"> &
  Partial<Pick<ChatActivity, "agentName" | "label" | "status">>;
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
  for (const status of ["waiting", "waiting_on_monitor"] as const) {
    assert.equal(
      chatWaitState(
        snapshot([lead, { ...child, status }], {
          monitors: [{ agent: "child", status: TaskStatus.running }],
        }),
        lead,
      ).live,
      true,
    );
  }
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
  const waitingLead: Agent = {
    ...lead,
    status: AgentStatus.waiting,
    inFlight: false,
  };
  const waitState = (
    agents: Agent[] = [waitingLead],
    runtime: Parameters<typeof componentSnapshot>[1] = {},
    agent = agents[0],
  ) => chatWaitState(snapshot(agents, runtime), required(agent));
  assert.equal(waitState().live, false);
  assert.equal(waitState().label, "Turn ended. Send a message to continue.");
  assert.deepEqual(waitState().commands, []);
  assert.deepEqual(waitState().monitors, []);
  for (const status of ["queued", "starting", "running", "approval"] as const) {
    const agents = [waitingLead, { ...child, status }];
    assert.equal(waitState(agents).label, "Waiting for 1 agent", status);
  }
  assert.equal(
    waitState([
      waitingLead,
      { ...child, status: AgentStatus.completed, inFlight: true },
    ]).live,
    true,
  );
  for (const status of [
    "idle",
    "completed",
    "failed",
    "paused",
    "interrupted",
  ] as const)
    assert.equal(
      waitState([waitingLead, { ...child, status, inFlight: false }]).live,
      false,
      status,
    );
  assert.equal(
    waitState([
      waitingLead,
      {
        ...child,
        source: "local" as NonNullable<Agent["source"]>,
        status: AgentStatus.running,
      },
    ]).live,
    false,
    "only managed children can keep a conversation waiting",
  );
  assert.equal(
    waitState([
      waitingLead,
      { ...child, status: AgentStatus.running, parentId: "other" },
    ]).live,
    false,
  );
  const ownCommand = {
    ...command,
    agent: "lead",
    kind: TaskKind.command,
  } satisfies Snapshot["runtime"]["tasks"][number];
  const ownMonitor: Snapshot["runtime"]["monitors"][number] = {
    id: "monitor",
    agent: "lead",
    command: "watch build",
    status: TaskStatus.running,
    created: 1,
    display: backgroundDisplay({ kind: "monitor", kindLabel: "monitor" }),
  };
  for (const status of ["starting", "running", "approval"] as const) {
    assert.equal(
      waitState(undefined, { tasks: [{ ...ownCommand, status }] }).label,
      "Waiting for 1 command",
    );
    assert.equal(
      waitState(undefined, { monitors: [{ ...ownMonitor, status }] }).label,
      "Waiting for 1 monitor",
    );
  }
  for (const record of [
    { status: "completed" },
    { status: "failed" },
    { agent: "other" },
  ] as const) {
    assert.equal(
      waitState(undefined, { tasks: [{ ...ownCommand, ...record }] }).live,
      false,
    );
    assert.equal(
      waitState(undefined, { monitors: [{ ...ownMonitor, ...record }] }).live,
      false,
    );
  }
  assert.equal(
    waitState(undefined, {
      tasks: [
        {
          ...ownCommand,
          command: undefined,
          kind: TaskKind.tool,
          display: backgroundDisplay({ kind: "tool", kindLabel: "tool" }),
          query: "search",
        },
      ],
    }).live,
    false,
  );
  assert.equal(
    chatWaitState(
      snapshot([{ ...waitingLead, inFlight: false }]),
      { ...waitingLead, inFlight: false },
      new Map<string, ActivityFixture[]>([
        [
          "lead",
          [
            {
              ...ownCommand,
              agentId: "lead",
              command: undefined,
              kind: "task",
              commandTask: true,
              background: true,
            },
          ],
        ],
      ]) as Map<string, ChatActivity[]>,
    ).commands.length,
    1,
    "a command task remains visible when its command text is unavailable",
  );
  const mismatchedRecordIsNotOwned = chatWaitState(
    snapshot([{ ...waitingLead, inFlight: false }]),
    { ...waitingLead, inFlight: false },
    new Map<string, ActivityFixture[]>([
      [
        "lead",
        [
          {
            ...ownCommand,
            agentId: "lead",
            kind: "agent",
            commandTask: true,
            background: true,
          },
        ],
      ],
    ]) as Map<string, ChatActivity[]>,
  );
  assert.deepEqual(mismatchedRecordIsNotOwned.commands, []);
  assert.equal(
    chatWaitState(
      snapshot([{ ...waitingLead, inFlight: false }]),
      { ...waitingLead, inFlight: false },
      new Map<string, ActivityFixture[]>([
        [
          "lead",
          [
            {
              ...ownCommand,
              agentId: "lead",
              kind: "task",
              commandTask: true,
              background: false,
            },
          ],
        ],
      ]) as Map<string, ChatActivity[]>,
    ).commands.length,
    1,
    "a stopped turn makes even a foreground task a live wait trigger",
  );
  assert.equal(
    waitState([{ ...waitingLead, inFlight: true, turnId: "current" }], {
      tasks: [{ ...ownCommand, turnId: "current" }],
    }).live,
    false,
    "A foreground tool is not a wake trigger after a turn",
  );
  const parked: Agent = {
    ...waitingLead,
    status: AgentStatus.parked,
    parkedEvent: "build-ready",
    autoWake: false,
  };
  assert.equal(waitState([parked]).label, "Waiting for event build-ready");
  const inputFields = { id: "input", agent: "lead", epoch: 2 };
  const input = simpleRequest(inputFields);
  assert.equal(
    waitState(undefined, { requests: [input] }).label,
    "Waiting for 1 input",
  );
  const withoutRequests = snapshot([waitingLead]);
  Reflect.deleteProperty(withoutRequests.runtime, "requests");
  assert.equal(
    chatWaitState(withoutRequests, waitingLead).inputs,
    0,
    "snapshots without a requests collection contain no input triggers",
  );
  assert.equal(
    chatWaitState(
      snapshot([waitingLead], {
        requests: [simpleRequest({ agent: "lead" })],
      }),
      waitingLead,
    ).inputs,
    1,
    "an unscoped request epoch remains current",
  );
  for (const record of [
    { answered: true },
    { deferred: true },
    { epoch: 1 },
    { agent: "other" },
  ] as const)
    assert.equal(
      waitState(undefined, {
        requests: [simpleRequest({ ...inputFields, ...record })],
      }).live,
      false,
    );
  const twoWorkers: Agent[] = [
    waitingLead,
    { ...child, status: AgentStatus.running },
    { ...child, id: "child-2", status: AgentStatus.queued },
  ];
  assert.equal(waitState(twoWorkers).label, "Waiting for 2 agents");
  assert.equal(
    waitState(twoWorkers, { monitors: [ownMonitor] }).label,
    "Waiting for 2 agents and 1 monitor",
  );
  assert.equal(
    waitState(twoWorkers, { tasks: [ownCommand], monitors: [ownMonitor] })
      .label,
    "Waiting for 2 agents, 1 command and 1 monitor",
  );
  assert.equal(
    waitState([{ ...waitingLead, parkedEvent: "ready" }], {
      tasks: [ownCommand, { ...ownCommand, id: "cmd-2" }],
      monitors: [ownMonitor, { ...ownMonitor, id: "monitor-2" }],
      requests: [input, simpleRequest({ ...inputFields, id: "input-2" })],
    }).label,
    "Waiting for 2 commands, 2 monitors, event ready and 2 inputs",
  );
  console.log(
    "PASS live wait triggers: workers, commands, monitors, event, input, exact counts, stale records, ownership, and static turn end",
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
it("keeps a parent waiting for a child with current command or monitor work", () => {
  const lead = makeAgent({ status: "waiting", inFlight: false });
  const child = makeAgent({
    id: "child",
    parentId: lead.id,
    rootId: lead.id,
    isLead: false,
    status: "waiting",
    inFlight: false,
  });
  for (const kind of ["monitors", "tasks"] as const) {
    const work: Snapshot["runtime"]["tasks"][number] = {
      id: "work",
      agent: child.id,
      status: TaskStatus.running,
      command: "watch build",
      created: 1,
      kind: TaskKind.command,
      display: backgroundDisplay(),
    };
    for (const status of ["starting", "running", "approval"] as const) {
      const snapshot = makeSnapshot([lead, child], {
        [kind]: [{ ...work, status }],
      });
      const wait = chatWaitState(snapshot, lead);
      assert.deepEqual(
        wait.agents.map(({ id }) => id),
        [child.id],
      );
      assert.deepEqual(wait.monitors, []);
      assert.deepEqual(wait.commands, []);
      assert.equal(chatWaitState(snapshot, child).live, true);
    }
    for (const change of [
      { status: "paused", autoWake: false },
      { status: "failed" },
      { status: "interrupted" },
      { status: "completed" },
      { status: "idle" },
      { archived: true },
      { autoWake: false },
      { source: "local" as NonNullable<Agent["source"]> },
    ] as const) {
      const snapshot = makeSnapshot([lead, { ...child, ...change }], {
        [kind]: [work],
      });
      assert.equal(chatWaitState(snapshot, lead).live, false);
    }
  }
  assert.equal(
    chatWaitState(
      makeSnapshot([lead, child], {
        tasks: [{ id: "tool", agent: child.id, status: TaskStatus.running }],
      }),
      lead,
    ).live,
    false,
    "an unfinished generic tool record does not promise another child turn",
  );
});
it("counts active work through waiting descendants without crossing stopped branches", () => {
  const lead = makeAgent({ status: "waiting", inFlight: false });
  const middle = makeAgent({
    id: "middle",
    parentId: lead.id,
    rootId: lead.id,
    isLead: false,
    status: "waiting",
    inFlight: false,
  });
  const child: Agent = { ...middle, id: "child", parentId: middle.id };
  const monitor = {
    id: "watch",
    agent: child.id,
    status: "running" as const,
    epoch: child.epoch,
  };
  const snapshot = makeSnapshot([lead, middle, child], { monitors: [monitor] });
  for (const agent of [lead, middle] as const) {
    assert.equal(
      required(chatWaitState(snapshot, agent).agents[0]).parentId,
      agent.id,
    );
  }
  for (const change of [
    { status: "paused", autoWake: false },
    { status: "failed" },
    { archived: true },
    { autoWake: false },
    { source: "local" as NonNullable<Agent["source"]> },
    { parentId: "other-root" },
  ] as const) {
    const snapshot = makeSnapshot([lead, { ...middle, ...change }, child], {
      monitors: [monitor],
    });
    assert.equal(chatWaitState(snapshot, lead).live, false);
  }
  for (const runtime of [
    {},
    { monitors: [{ ...monitor, status: "completed" }] },
  ] as const) {
    const snapshot = makeSnapshot([lead, middle, child], runtime);
    for (const agent of [lead, middle, child] as const)
      assert.equal(chatWaitState(snapshot, agent).live, false);
  }
  const sibling: Agent = { ...child, id: "sibling", parentId: lead.id };
  assert.equal(
    chatWaitState(
      makeSnapshot([lead, middle, child, sibling], {
        monitors: [{ ...monitor, agent: sibling.id }],
      }),
      middle,
    ).live,
    false,
    "a root activity does not become work owned by a sibling branch",
  );
  assert.equal(
    chatWaitState(
      makeSnapshot([lead, { ...middle, parentId: child.id }, child], {
        monitors: [monitor],
      }),
      middle,
    ).agents.length,
    1,
    "a parent cycle cannot include the requesting agent again",
  );
});
it("keeps the parent waiting for a parked child's saved event", () => {
  const lead = makeAgent({ status: "waiting", inFlight: false });
  const child = makeAgent({
    id: "child",
    parentId: lead.id,
    rootId: lead.id,
    status: "parked",
    inFlight: false,
    autoWake: false,
    parkedEvent: "build-ready",
  });
  const monitor = {
    id: "watch",
    agent: child.id,
    status: "running" as const,
    epoch: 2,
  };
  for (const monitors of [[], [monitor]] as const) {
    const snapshot = makeSnapshot([lead, child], { monitors });
    assert.equal(chatWaitState(snapshot, child).event, "build-ready");
  }
  for (const change of [
    { parkedEvent: null },
    { parkedEvent: "" },
    { status: "waiting" },
    { status: "paused" },
    { status: "failed" },
    { status: "interrupted" },
    { archived: true },
  ] as const)
    assert.equal(
      chatWaitState(
        makeSnapshot([lead, { ...child, ...change }], { monitors: [monitor] }),
        lead,
      ).live,
      false,
    );
});
it("rejects malformed request epochs while preserving absent, null and numeric rules", () => {
  const waiting = makeAgent({ status: "waiting", epoch: 4 });
  const requestEpochs = [
    [{}, true],
    [{ epoch: null }, true],
    [{ epoch: 4 }, true],
    [{ epoch: 3 }, false],
  ] as const;
  for (const [record, current] of requestEpochs) {
    const request = simpleRequest({ agent: "lead", ...record });
    assert.equal(
      chatWaitState(makeSnapshot([waiting], { requests: [request] }), waiting)
        .inputs,
      current ? 1 : 0,
      `wait input epoch ${JSON.stringify(record)}`,
    );
  }
});
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
it("builds live wait labels only from current owned triggers", () => {
  const lead = makeAgent({
    status: "parked",
    inFlight: false,
    parkedEvent: "release-ready",
  });
  const child = makeAgent({
    id: "child",
    rootId: "lead",
    parentId: "lead",
    status: "approval",
    inFlight: true,
  });
  // Partial fixture omits display-only agentName and label.
  const task = {
    id: "command",
    kind: "task",
    commandTask: true,
    agentId: "lead",
    background: true,
    status: "running",
  } as ChatActivity;
  // Partial fixture omits display-only agentName and label.
  const monitor = {
    id: "watch",
    kind: "monitor",
    agentId: "lead",
    status: "approval",
  } as ChatActivity;
  const snapshot = makeSnapshot([lead, child], {
    requests: [
      simpleRequest({ id: "pending", agent: "lead", epoch: 2 }),
      simpleRequest({
        id: "deferred",
        agent: "lead",
        deferred: true,
      }),
      simpleRequest({ id: "answered", agent: "lead", answered: true }),
      simpleRequest({ id: "stale", agent: "lead", epoch: 1 }),
      simpleRequest({ id: "child", agent: "child" }),
    ],
  });
  const state = chatWaitState(
    snapshot,
    lead,
    new Map<string, ActivityFixture[]>([
      [
        "lead",
        [task, monitor, { ...task, id: "other-agent", agentId: "child" }],
      ],
    ]) as Map<string, ChatActivity[]>,
  );
  assert.equal(state.live, true);
  assert.equal(
    state.label,
    "Waiting for 1 agent, 1 command, 1 monitor, event release-ready and 1 input",
  );
  assert.deepEqual(
    state.agents.map(({ id }) => id),
    ["child"],
  );
  assert.deepEqual(
    state.commands.map(({ id }) => id),
    ["command"],
  );
  assert.deepEqual(
    state.monitors.map(({ id }) => id),
    ["watch"],
  );
  assert.equal(state.event, "release-ready");
  assert.equal(state.inputs, 1);
  const foreground = { ...task, id: "foreground", background: false };
  assert.equal(
    chatWaitState(
      makeSnapshot([makeAgent({ inFlight: true })]),
      makeAgent({ inFlight: true }),
      new Map<string, ActivityFixture[]>([["lead", [foreground]]]) as Map<
        string,
        ChatActivity[]
      >,
    ).live,
    false,
  );
  const noEpochAgent = makeAgent({ epoch: null, status: "waiting" });
  assert.equal(
    chatWaitState(
      makeSnapshot([noEpochAgent], {
        requests: [simpleRequest({ agent: "lead", epoch: 7 })],
      }),
      noEpochAgent,
    ).label,
    "Waiting for 1 input",
  );
  const scopedAgent = makeAgent({ status: "waiting", epoch: 2 });
  assert.equal(
    chatWaitState(
      makeSnapshot([scopedAgent], {
        requests: [simpleRequest({ agent: "lead" })],
      }),
      scopedAgent,
    ).inputs,
    1,
  );
  const waitingChild = makeAgent({
    id: "wait-child",
    parentId: "lead",
    status: "completed",
    inFlight: true,
  });
  assert.equal(
    chatWaitState(makeSnapshot([lead, waitingChild]), lead).label,
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

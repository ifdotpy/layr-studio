import { ChatIndicatorKind as ChatOperationalKind } from "../../../status/operationalStatus.ts";
import { AgentStatus } from "../../../generated/api.ts";
import { required } from "../../conversation/ComposerAttachments/componentTestFixtures.ts";
import type { Agent } from "../../../types.ts";
import type { ChatIndicator } from "../../../status/operationalStatus.ts";
import assert from "node:assert/strict";
import { it } from "vitest";

it("orders actionable indicators before work and keeps completed results collapsed", async () => {
  const { teamDispatchGroups } = await import("./TeamSummary.tsx");
  const rows: Agent[] = [
    { id: "work", status: AgentStatus.running },
    { id: "answer", status: AgentStatus.running, inFlight: true },
    { id: "failed", status: AgentStatus.failed },
    { id: "unread", status: AgentStatus.completed },
    { id: "read", status: AgentStatus.completed },
    { id: "paused", status: AgentStatus.paused },
  ];
  const indicators = new Map<string, ChatIndicator>([
    ["work", { kind: ChatOperationalKind["working"], label: "Working" }],
    [
      "answer",
      { kind: ChatOperationalKind["answer"], label: "Needs your answer" },
    ],
    ["failed", { kind: ChatOperationalKind["error"], label: "Failed" }],
    ["unread", { kind: ChatOperationalKind["unread"], label: "Unread result" }],
    ["read", { kind: ChatOperationalKind["none"], label: "Read" }],
    ["paused", { kind: ChatOperationalKind["paused"], label: "Stopped" }],
  ]);
  const { groups, completed } = teamDispatchGroups(rows, indicators);
  assert.deepEqual(
    groups.filter((group) => group.workers.length).map((group) => group.kind),
    ["answer", "error", "working", "paused"],
  );
  assert.deepEqual(
    completed.map((row) => row.id),
    ["unread", "read"],
  );
  assert.equal(
    required(required(groups[0]).workers[0]).id,
    "answer",
    "an in-flight question stays actionable",
  );
});

it("derives hierarchy from parent identities and terminates on cycles", async () => {
  const { workerDepth } = await import("./TeamSummary.tsx");
  const agents = new Map<string, Agent>([
    ["lead", { id: "lead" }],
    ["worker", { id: "worker", parentId: "lead" }],
    ["reviewer", { id: "reviewer", parentId: "worker" }],
  ]);
  assert.equal(workerDepth(agents.get("lead")!, agents), 0);
  assert.equal(workerDepth(agents.get("reviewer")!, agents), 2);
  agents.set("lead", { id: "lead", parentId: "reviewer" });
  assert.equal(workerDepth(agents.get("reviewer")!, agents), 2);
});

it("keeps parents before their children without losing orphaned or cyclic rows", async () => {
  const { orderWorkerHierarchy } = await import("./TeamSummary.tsx");
  const agents: Agent[] = [
    { id: "child", parentId: "parent" },
    { id: "other" },
    { id: "parent" },
    { id: "cycle-a", parentId: "cycle-b" },
    { id: "cycle-b", parentId: "cycle-a" },
  ];
  const ordered = orderWorkerHierarchy(agents).map((agent) => agent.id);
  assert.deepEqual(ordered, ["other", "parent", "child", "cycle-a", "cycle-b"]);
});

it("filters the same canonical lanes counted by the summary, including queued work", async () => {
  const { filterTeamWorkers } = await import("./TeamSummary.tsx");
  const rows: Agent[] = [
    { id: "queued", status: AgentStatus.queued },
    { id: "running", status: AgentStatus.running, inFlight: true },
    { id: "error", status: AgentStatus.failed },
    { id: "paused", status: AgentStatus.paused },
    { id: "done", status: AgentStatus.completed },
  ];
  const indicators = new Map<string, ChatIndicator>([
    ["queued", { kind: ChatOperationalKind["working"], label: "Queued" }],
    ["running", { kind: ChatOperationalKind["working"], label: "Working" }],
    ["error", { kind: ChatOperationalKind["error"], label: "Failed" }],
    ["paused", { kind: ChatOperationalKind["paused"], label: "Stopped" }],
    ["done", { kind: ChatOperationalKind["unread"], label: "New result" }],
  ]);
  const ids = (filter: string) =>
    filterTeamWorkers(rows, indicators, filter).map((a) => a.id);
  assert.deepEqual(ids("active"), ["queued", "running"]);
  assert.deepEqual(ids("attention"), ["error"]);
  assert.deepEqual(ids("paused"), ["paused"]);
  assert.deepEqual(ids("completed"), ["done"]);
  assert.strictEqual(filterTeamWorkers(rows, indicators, "all"), rows);
});

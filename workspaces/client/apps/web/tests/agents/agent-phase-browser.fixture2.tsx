import type { ComponentSnapshot as Snapshot } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import { backgroundDisplay } from "../fixtures/background.ts";
import { useState } from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import AgentPhase from "../../src/components/agents/AgentPhase/AgentPhase.tsx";
import ChatStatus from "../../src/components/agents/ChatStatus/ChatStatus.tsx";
import type { components } from "../../src/generated/api.ts";
import type { Agent, BackgroundTask, Monitor } from "../../src/types.ts";
import { simpleRequest } from "../../src/approvalFixtures.ts";
import type { Approval } from "../../src/approvals.ts";
import "../../src/style.css";
import "../../src/workspace-layout.css";

declare global {
  interface Window {
    base: Agent;
    worker: (index: number) => Agent;
    command: (index: number) => BackgroundTask;
    monitor: (index: number) => Monitor;
    input: (id: string) => Approval;
    setData: (value: PhaseSnapshot) => void;
  }
}
const lead = {
  id: "lead",
  name: "Main",
  rootId: "lead",
  source: "managed",
  status: "waiting",
  inFlight: false,
  epoch: 2,
} satisfies Agent;
window.base = lead;
window.worker = (i) => ({
  ...lead,
  id: "child-" + i,
  name: "Worker " + i,
  parentId: "lead",
  status: "running",
  inFlight: true,
});
window.command = (i) => ({
  created: 1,
  display: backgroundDisplay("command", "npm run check " + i),
  id: "cmd-" + i,
  agent: "lead",
  kind: "command",
  command: "npm run check " + i,
  status: "running",
  epoch: 2,
});
window.input = (id) => simpleRequest({ id, agent: "lead", epoch: 2 });
window.monitor = (i) => ({
  created: 1,
  display: backgroundDisplay("monitor", "watch build " + i),
  id: "monitor-" + i,
  agent: "lead",
  command: "watch build " + i,
  status: "running",
  epoch: 2,
});
export type PhaseSnapshot = Pick<Snapshot, "threads"> & {
  operationalStatus: components["schemas"]["AgentOperationalStatus"];
  runtime: Pick<Snapshot["runtime"], "requests" | "tasks" | "monitors">;
};
const initial: PhaseSnapshot = {
  operationalStatus: {
    reason: "waiting",
    wait: { agents: 2, commands: 0, monitors: 0, inputs: 0, event: null },
  },
  threads: [lead, window.worker(1), window.worker(2)],
  runtime: { requests: [], tasks: [], monitors: [] },
};

function Fixture() {
  const [data, setData] = useState<PhaseSnapshot>(initial);
  window.setData = (value) => flushSync(() => setData(value));
  const agent = data.threads[0]!;
  const fixture = observableFixture(
    data.threads.map((row) =>
      row.id === agent.id
        ? { ...row, operationalStatus: data.operationalStatus }
        : row,
    ),
  );
  fixture.processes.apply({
    processes: [],
    activities: [
      ...data.runtime.tasks.map((task) => ({
        source: "task" as const,
        value: {
          agent: task.agent || "lead",
          command: task.command,
          created: task.created || 0,
          display: task.display,
          id: task.id,
          kind: task.kind || ("command" as const),
          name: task.name || "",
          status: task.status,
          turnId: task.turnId || "",
        },
      })),
      ...data.runtime.monitors.map((monitor) => ({
        source: "process" as const,
        value: {
          agent: monitor.agent || "lead",
          accountKey: "default",
          bytes: 0,
          cancelRequested: false,
          command: monitor.command,
          created: monitor.created,
          display: monitor.display,
          epoch: 0,
          generation: monitor.id,
          id: monitor.id,
          key: monitor.id,
          name: monitor.name || "",
          outputPatterns: [],
          outputTruncated: false,
          source: "monitor" as const,
          status: monitor.status,
          tail: "",
        },
      })),
    ],
  });
  const model = fixture.mirror.current.agents.get(agent.id)!;
  return (
    <fixture.Provider>
      <main style={{ width: "100%", padding: 16, marginTop: 260 }}>
        <div id="indicator">
          <ChatStatus agent={model} />
        </div>
        <AgentPhase agent={model} connection={null} />
        <div id="anchor">Next content</div>
      </main>
    </fixture.Provider>
  );
}
const root = document.getElementById("root")!;
createRoot(root).render(<Fixture />);

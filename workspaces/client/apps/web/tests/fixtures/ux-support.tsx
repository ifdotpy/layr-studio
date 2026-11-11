const fixtureServerStore = createServerStore(defaultSubscriptions);
window.addEventListener("pagehide", () => fixtureServerStore.dispose(), {
  once: true,
});
import { createServerStore } from "../../src/store/server.ts";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events.ts";
import { theme } from "../../src/theme.ts";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { backgroundDisplay } from "./background.ts";
import React, { useEffect, useState } from "react";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import "../../src/studio-theme.css";
import "../../src/appearance.css";
import { FixtureAreas } from "./owner-support.tsx";
import TerminalDock from "../../src/components/shell/TerminalDock/TerminalDock.tsx";
import { BackgroundTasksFixture as BackgroundTasks } from "../fixtures/owner-support.tsx";
import DraftVersions from "../../src/components/shell/DraftVersions/DraftVersions.tsx";
import type { components } from "../../src/generated/api.ts";
import type { Agent } from "../../src/types.ts";
import { fixtureSnapshot } from "./snapshot.ts";

declare global {
  interface Window {
    support: { initial: () => void; newer: () => void; scope: () => void };
  }
}

const task = (id: string, created: number, agent = "lead") =>
  ({
    id,
    created,
    agent,
    display: backgroundDisplay("command", id),
    kind: "command",
    name: "commandExecution",
    command: id,
    status: "running",
    tail: id,
    turnId: "turn",
  }) satisfies components["schemas"]["TaskProjection"];
function Fixture() {
  const [scope, setScope] = useState("lead");
  const [tasks, setTasks] = useState<ReturnType<typeof task>[]>([]);
  const [text, setText] = useState("Current");
  const [opened, setOpened] = useState(false);
  const agent = {
    id: "lead",
    rootId: "lead",
    isLead: true,
    name: "Lead",
    cwd: "/fixture",
  } satisfies Agent;
  const data = fixtureSnapshot(
    [agent, { ...agent, id: "other", rootId: "other" }],
    tasks,
  );
  useEffect(() => {
    defaultSubscriptions.publish(
      { kind: "state" },
      data.threads.map((owner) => ({
        collection: "agentSummary",
        id: owner.id,
        value: agentSummaryFixture(owner.id, { name: owner.name ?? owner.id }),
      })),
    );
    defaultSubscriptions.publish(
      { kind: "processes", agentId: scope },
      {
        processes: [],
        activities: tasks.map((value) => ({ source: "task" as const, value })),
      },
    );
  }, [scope, tasks]);
  window.support = {
    initial: () => setTasks([task("first", 10)]),
    newer: () => setTasks([task("new", 20), task("first", 10)]),
    scope: () => {
      setScope("other");
      setTasks([task("other-task", 30, "other")]);
    },
  };
  return (
    <MantineProvider theme={theme}>
      <button onClick={() => setOpened(true)}>Tasks</button>
      <BackgroundTasks
        opened={opened}
        close={() => setOpened(false)}
        leadId={scope}
        openAgent={() => {}}

        notify={() => {}}
      />
      <textarea
        aria-label="Draft text"
        value={text}
        onChange={(e) => setText(e.target.value)}
      />
      <DraftVersions
        versions={[
          {
            id: "branch",
            session: "lead",
            text: "Saved text",
            device: "recorded-device",
            updated: 1725926400000,
          },
          {
            id: "branch:conflict",
            session: "lead",
            text: "Alternative text",
            device: "recorded-device",
            updated: 1725926400000,
          },
        ]}
        useVersion={(version, mode) =>
          setText((previous) =>
            mode === "append" ? previous + "\n\n" + version.text : version.text,
          )
        }
        dismiss={() => {}}
      />
      <FixtureAreas selectedId={agent.id}>
        <TerminalDock />
      </FixtureAreas>
    </MantineProvider>
  );
}
createRoot(document.getElementById("root")!).render(
  <ServerStoreProvider value={fixtureServerStore}>
    <Fixture />
  </ServerStoreProvider>,
);

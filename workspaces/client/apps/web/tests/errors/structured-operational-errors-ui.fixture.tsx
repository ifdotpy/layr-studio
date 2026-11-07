import { RequestFixture } from "../../src/components/questions/requestFixtures";
import { ServerViewStoreProvider } from "../../src/servers/ServerServicesContext";
import { ApiError } from "../../src/api";
import { flushSync } from "react-dom";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture";
import type { components } from "../../src/generated/api";
import { approval } from "../../src/approvalFixtures.ts";
import { desktopFixture } from "../files/desktop.fixture.ts";
import type { JsonInput } from "../../src/types.ts";
import type { AgentEntityDto as Agent } from "../../src/generated/api.ts";
import { fixtureSnapshot } from "../fixtures/snapshot.ts";
declare global {
  interface Window {
    readRecordings: () => ReturnType<typeof listRecordings>;
    crashes: string[];
    renderCase: (which: string, value: JsonInput) => Promise<void>;
    transcriptionCalls: number;
  }
}

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import Requests, {
  RequestArea,
} from "../../src/components/questions/Requests.tsx";
import UserMessages from "../../src/components/shell/messages/UserMessages.tsx";
import BackgroundTasks from "../../src/components/shell/BackgroundTasks.tsx";
import CapacityRetry from "../../src/components/CapacityRetry.tsx";
import SafetyBuffering from "../../src/components/conversation/transcript/SafetyBuffering.tsx";
import { Dictation } from "../../src/components/Dictation.tsx";
import { saveRecording, listRecordings } from "../../src/dictation/storage.ts";
window.readRecordings = () => listRecordings("lead");
const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("Missing fixture root");
const root = createRoot(rootElement);
const agent: Agent = {
  id: "lead",
  rootId: "lead",
  name: "Lead",
  epoch: 0,
  accountKey: "default",
  turnId: "turn",
  inFlight: true,
};
const snapshot = { ...fixtureSnapshot([agent]), stateDir: "fixture" };
const noop = () => {};
const refresh = async () => {};
let renderId = 0;
class Boundary extends React.Component<React.PropsWithChildren> {
  override state = { error: "" };
  static getDerivedStateFromError(error: Error) {
    return { error: error.message };
  }
  override componentDidCatch(error: Error) {
    window.crashes.push(error.message);
  }
  override render() {
    return this.state.error
      ? React.createElement("p", { "data-crash": true }, this.state.error)
      : this.props.children;
  }
}
window.renderCase = async (which, value) => {
  let component: React.ReactNode;
  if (which === "request") {
    const request = approval({
      id: "approval",
      agent: "lead",
      request: {
        kind: "exec",
        command: "",
        cwd: "",
        reason: "Structured operation failed",
      },
    });
    component = React.createElement(
      RequestFixture,
      {
        requests: [request],
        agents: [
          agentSummaryFixture(agent.id, {
            name: agent.name,
            rootId: agent.rootId,
          }),
        ],
        details: [{ id: agent.id }],
      },
      React.createElement(
        RequestArea,
        { scope: "fixture", refresh: async () => {}, notify: () => {} },
        React.createElement(Requests, { all: true }),
      ),
    );
  }
  if (which === "message")
    component = React.createElement(UserMessages, {
      data: {
        ...snapshot,
        runtime: {
          ...snapshot.runtime,
          complaints: [
            {
              id: "message",
              leadId: "lead",
              author: "lead",
              authorName: "Lead",
              recipient: "user",
              title: "User check",
              created: 1,
              version: 1,
            },
          ],
        },
      },
      refresh,
      notify: noop,
    });
  if (which === "background") {
    const task: components["schemas"]["TaskProjection"] = {
      id: "tool",
      agent: "lead",
      kind: "command",
      name: "commandExecution",
      command: "fixture command",
      status: "running",
      created: 1,
      turnId: "turn",
      display: {
        kind: "command",
        kindLabel: "command",
        title: "fixture command",
        active: true,
        indicator: "working",
        label: "running",
        inBackgroundList: true,
        actions: ["readOutput"],
      },
      // @ts-expect-error error deliberately contains a structured legacy diagnostic.
      error: value,
      stdinError: value,
    };
    defaultSubscriptions.publish({ kind: "state" }, [
      {
        collection: "agentSummary",
        id: "lead",
        value: agentSummaryFixture("lead"),
      },
    ]);
    defaultSubscriptions.publish(
      { kind: "processes", agentId: "lead" },
      { processes: [], activities: [{ source: "task", value: task }] },
    );
    component = React.createElement(BackgroundTasks, {
      opened: true,
      close: noop,
      leadId: "lead",
      openAgent: noop,
      refresh,
      notify: noop,
    });
  }
  if (which === "capacity")
    component = React.createElement(CapacityRetry, {
      agentId: "lead",
      retry: {
        id: "retry",
        status: "scheduled",
        dueAt: Date.now() / 1000 + 100,
        // @ts-expect-error reason deliberately contains a structured legacy diagnostic.
        reason: value,
      },
    });
  if (which === "safety")
    component = React.createElement(SafetyBuffering, {
      agent: {
        ...agent,
        nativeSafetyRetry: {
          epoch: 0,
          accountKey: "default",
          turnId: "turn",
          stage: "failed",
          // @ts-expect-error error deliberately contains a structured legacy diagnostic.
          error: value,
        },
      },
    });
  if (which === "dictation") {
    window.codexDesktop = desktopFixture();
    await saveRecording({
      id: "recording",
      chatId: "lead",
      state: "ready",
      sampleRate: 16000,
      samples: 4,
      created: Date.now(),
      // @ts-expect-error error deliberately contains a structured legacy diagnostic.
      error: value,
    });
    component = React.createElement(Dictation, {
      chatId: "lead",
      onInsert: noop,
    });
  }
  flushSync(() =>
    root.render(
      React.createElement(
        MantineProvider,
        null,
        React.createElement(
          ServerViewStoreProvider,
          null,
          React.createElement(Boundary, { key: ++renderId }, component),
        ),
      ),
    ),
  );
  if (which === "message")
    defaultSubscriptions.fail(
      { kind: "complaint", complaintId: "message" },
      new ApiError(value, 503, value),
    );
};
window.crashes = [];

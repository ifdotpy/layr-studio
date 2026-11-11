const fixtureServerStore = createServerStore(defaultSubscriptions);
window.addEventListener("pagehide", () => fixtureServerStore.dispose(), {
  once: true,
});
import { createServerStore } from "../../src/store/server.ts";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events.ts";
import "../../src/rendererStyles.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { ConversationProvider } from "../../src/components/conversation/support/navigation.ts";
import { conversationActionsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import {
  RequestFixture,
  createRequestFixture,
} from "../../src/components/conversation/questions/support/requestFixtures.ts";
import { ApiError } from "../../src/api.ts";
import { flushSync } from "react-dom";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
import type { components } from "../../src/generated/api.ts";
import { approval } from "../../src/approvalFixtures.ts";
import { desktopFixture } from "../files/desktop.fixture.ts";
import type { JsonInput } from "../../src/types.ts";
import type { AgentEntityDto as Agent } from "../../src/generated/api.ts";
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
import Requests from "../../src/components/conversation/questions/Requests/Requests.tsx";
import UserMessages from "../../src/components/shell/messages/UserMessages/UserMessages.tsx";
import { BackgroundTasksFixture as BackgroundTasks } from "../fixtures/owner-support.tsx";
import CapacityRetry from "../../src/components/conversation/CapacityRetry/CapacityRetry.tsx";
import SafetyBuffering from "../../src/components/conversation/transcript/SafetyBuffering/SafetyBuffering.tsx";
import { Dictation } from "../../src/components/conversation/Dictation/Dictation.tsx";
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
const models = observableFixture([agent]);
const noop = () => {};
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
let requestOwner: ReturnType<typeof createRequestFixture> | undefined;
window.addEventListener("pagehide", () => requestOwner?.dispose(), {
  once: true,
});
window.renderCase = async (which, value) => {
  requestOwner?.dispose();
  requestOwner = undefined;
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
    requestOwner = createRequestFixture({
      scope: "fixture",
      notify: () => {},
      requests: [request],
      agents: [
        agentSummaryFixture(agent.id, {
          name: agent.name,
          rootId: agent.rootId,
        }),
      ],
      details: [{ id: agent.id }],
    });
    requestOwner.start();
    component = React.createElement(
      RequestFixture,
      { owner: requestOwner },
      React.createElement(Requests, { all: true }),
    );
  }
  if (which === "message") {
    models.apply([
      {
        collection: "workspace",
        id: "workspace",
        value: { stateDir: "fixture" },
      },
      {
        collection: "complaint",
        id: "message",
        value: {
          id: "message",
          leadId: "lead",
          author: "lead",
          authorName: "Lead",
          recipient: "user",
          title: "User check",
          created: 1,
          version: 1,
        },
      },
    ]);
    component = React.createElement(
      ServerStoreProvider,
      {
        value: {
          ...models.store,
          complaint: () => ({
            current: {
              status: "error" as const,
              error: new ApiError(value, 503, value),
            },
          }),
        },
      },
      React.createElement(
        ConversationProvider,
        { value: conversationActionsFixture() },
        React.createElement(UserMessages),
      ),
    );
  }
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
          ServerStoreProvider,
          { value: fixtureServerStore },
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

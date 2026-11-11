import "../../src/rendererStyles.ts";
import { AgentModelsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { ComponentProps } from "react";
import type { Agent, Message } from "../../src/types.ts";

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import History from "../../src/components/conversation/transcript/TurnHistory/TurnHistory.fixture.tsx";
function TurnHistory(props: ComponentProps<typeof History>) {
  return (
    <AgentModelsFixture agents={props.agent ? [props.agent] : []}>
      <History {...props} />
    </AgentModelsFixture>
  );
}
import ClaudeRepairNotice from "../../src/components/conversation/ClaudeRepairNotice/ClaudeRepairNotice.tsx";
import { NativeNotice } from "../../src/components/shell/NativeNotice/NativeNotice.tsx";
import { ClaudeRepairReportStore } from "../../src/components/conversation/ClaudeRepairNotice/ClaudeRepairReportStore.ts";
import * as api from "../../src/servers/net/api.ts";
import { observer } from "mobx-react-lite";
const RepairNotice = observer(ClaudeRepairNotice);
const agent: Agent = {
  id: "agent",
  threadId: "thread",
  provider: "claude",
  status: "paused",
  epoch: 1,
  error: "CLAUDE_SESSION_REPAIR_REQUIRED: needs repair",
};
const items: Message[] = [
  {
    id: "input",
    role: "user",
    text: "Update the report.",
    turnId: "turn",
    turnStatus: "uncertain",
  },
  {
    id: "answer",
    role: "assistant",
    text: "I read the source files.",
    turnId: "turn",
    turnStatus: "uncertain",
    turnError: {
      message:
        "Studio could not confirm the Claude turn outcome. The native transcript is incomplete.",
    },
  },
];
const repair = new ClaudeRepairReportStore(api, () => agent, false);
await repair.refresh();
window.addEventListener("pagehide", () => repair.dispose(), { once: true });
createRoot(document.getElementById("root")!).render(
  React.createElement(
    MantineProvider,
    null,
    React.createElement(
      "main",
      { style: { maxWidth: 900, margin: "auto", padding: 12 } },
      React.createElement("h1", null, "Conversation"),
      React.createElement(RepairNotice, { agent, store: repair }),
      React.createElement(TurnHistory, {
        items,
        agent,
        agentId: "agent",
        enabled: true,
        storageKey: "repair-fixture",
        onJump: () => {},
        renderMessage: (item) =>
          item.nativeNotice
            ? React.createElement(NativeNotice, { item })
            : React.createElement("p", null, item.text),
      }),
    ),
  ),
);

import {
  AgentSource,
  AgentStatus,
  ProviderKind,
} from "../../../generated/api.ts";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import AgentPhaseView from "./AgentPhase.tsx";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import { useMemo } from "react";
import type { MessageConnection } from "../../../hooks/messageConnection.ts";
function AgentPhase({
  agent,
  connection,
}: {
  agent: Agent;
  connection: MessageConnection | null;
}) {
  const fixture = useMemo(() => observableFixture([agent]), [agent]);
  return (
    <fixture.Provider>
      <AgentPhaseView
        agent={fixture.mirror.current.agents.get(agent.id)!}
        connection={connection}
      />
    </fixture.Provider>
  );
}
import type { Agent } from "../../../types.ts";
import { selectMessageConnection } from "../../../hooks/messageConnection.ts";

const meta = {
  title: "Agents/Status",
  component: AgentPhase,
  args: { connection: selectMessageConnection("live", "open") },
} satisfies Meta<typeof AgentPhase>;

export default meta;
type Story = StoryObj<typeof meta>;

const thinkingAgent = {
  id: "worker-1",
  name: "Transcript worker",
  source: AgentSource.managed,
  model: "gpt-6",
  status: AgentStatus.running,
  provider: ProviderKind.codex,
  created: 1,
  activity: { phase: "thinking" },
  inFlight: true,
  operationalStatus: { reason: "working" },
} as Agent;

export const Thinking: Story = {
  args: { agent: thinkingAgent },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    expect(canvas.getByText("Thinking", { exact: true })).toBeVisible();
  },
};

export const WaitingForAnswer: Story = {
  args: {
    agent: {
      ...thinkingAgent,
      status: AgentStatus["approval"],
      operationalStatus: { reason: "needs_input", wait: null },
      activity: undefined,
      inFlight: false,
      provider: ProviderKind["claude"],
      model: "claude-sonnet",
    },
  },
  play: async ({ canvasElement }) => {
    expect(
      within(canvasElement).getByText("Waiting for your answer", {
        exact: true,
      }),
    ).toBeVisible();
  },
};

export const Reconnecting: Story = {
  args: {
    agent: { ...thinkingAgent, activity: { phase: "tool" } },
    connection: selectMessageConnection("degraded", "connecting", {
      failure: "stream-error",
    }),
  },
  play: async ({ canvasElement }) => {
    expect(
      within(canvasElement).getByText("Connection failed. Reconnecting…", {
        exact: true,
      }),
    ).toBeVisible();
  },
};

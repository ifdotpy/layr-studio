import {
  observableFixture,
  conversationActionsFixture,
} from "../observableFixture.tsx";
import { ConversationProvider } from "../conversation/navigation.ts";
import { useMemo } from "react";
import {
  AgentSource as GeneratedApiAgentSource,
  AgentStatus as GeneratedApiAgentStatus,
} from "../../generated/api.ts";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import ConversationTitle from "./ConversationTitle";
import type { Agent } from "../../types";

const lead = {
  id: "lead-1",
  name: "Main agent",
  source: GeneratedApiAgentSource.managed,
  status: GeneratedApiAgentStatus.running,
} as Agent;

const navigation = conversationActionsFixture(fn());
const meta = {
  title: "Shell/Conversation title",
  component: ConversationTitle,
  decorators: [
    (Story, context) => {
      const fixture = useMemo(
        () =>
          observableFixture([
            {
              ...lead,
              name: context.args.title,
              isLead: context.name !== "Agent Conversation",
              rootId: "parent",
            },
          ]),
        [context.args.title, context.name],
      );
      return (
        <fixture.Provider>
          <ConversationProvider value={navigation}>
            <Story />
          </ConversationProvider>
        </fixture.Provider>
      );
    },
  ],
} satisfies Meta<typeof ConversationTitle>;

export default meta;
type Story = StoryObj<typeof meta>;

export const ActiveConversation: Story = {
  args: {
    title: "Refactor the conversation view",
    agentId: lead.id,
    serverAlias: "MAC",
  },
  play: async ({ canvasElement }) => {
    expect(
      within(canvasElement).getByRole("heading", { level: 1 }),
    ).toHaveTextContent("Refactor the conversation view");
    await expect(
      canvasElement.querySelector("#conversation-status"),
    ).toBeNull();
  },
};

export const AgentConversation: Story = {
  args: {
    ...ActiveConversation.args,
    title: "Build transcript components",
  },
  play: async ({ canvasElement }) => {
    await userEvent.click(
      within(canvasElement).getByRole("button", { name: "Back to main agent" }),
    );
    await expect(navigation.open).toHaveBeenCalledOnce();
  },
};

export const OfflineServer: Story = {
  args: { ...ActiveConversation.args, serverOffline: true },
  play: async ({ canvasElement }) => {
    const server = within(canvasElement).getByLabelText("MAC offline");
    await expect(server).toBeVisible();
    const line = server.querySelector(".chat-server-line")!;
    const reference = document.createElement("span");
    reference.style.color = "var(--studio-state-error)";
    server.append(reference);
    await expect(getComputedStyle(line).color).toBe(
      getComputedStyle(reference).color,
    );
    reference.remove();
  },
};

export const WaitingConversation: Story = {
  args: {
    ...ActiveConversation.args,
  },
  play: async ({ canvasElement }) => {
    await expect(canvasElement).not.toHaveTextContent(
      /Waiting|Working|monitor/i,
    );
    await expect(
      within(canvasElement).getByRole("heading", { level: 1 }),
    ).toBeVisible();
  },
};

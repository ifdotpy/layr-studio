import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";
import MessageQueue from "./MessageQueue.tsx";
import {
  createMessageQueueStore,
  type QueueProps,
} from "./MessageQueueStore.ts";
const defaults: QueueProps = {
  scope: "storybook-queue",
  items: [
    {
      id: "queued-1",
      text: "Review the request identity before the next turn.",
      created: 1700000000,
      status: "queued",
      kind: "user",
    },
    {
      id: "queued-2",
      text: "Check that long drafts recover after reconnecting to the project.",
      created: 1700000010,
      status: "queued",
      kind: "user",
    },
  ],
  onEdit: fn(),
  onCancel: fn(),
  onReorder: fn(),
  onSendNow: fn(),
  canReorder: true,
};
const store = (overrides: Partial<QueueProps> = {}) =>
  createMessageQueueStore(() => ({ ...defaults, ...overrides }), "storybook");
const meta = {
  title: "Agents/Message queue",
  component: MessageQueue,
  args: { store: store() },
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof MessageQueue>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Queued: Story = {};
export const Checking: Story = { args: { store: store({ loading: true }) } };
export const DeliveryError: Story = {
  args: {
    store: store({
      error: "Queue could not be refreshed. Saved edits remain available.",
    }),
  },
};

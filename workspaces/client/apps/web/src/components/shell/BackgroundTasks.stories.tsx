import {
  BackgroundTaskIndicator,
  BackgroundTaskKind,
  BackgroundTaskKindLabel,
  BackgroundTaskLabel,
  MonitorState,
} from "../../generated/api";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import { BackgroundTaskStrip } from "./BackgroundTasks";
import type { components } from "../../generated/api";
const task: components["schemas"]["MonitorEntityDto"] = {
  id: "monitor",
  agent: "worker",
  created: 1700000000,
  status: MonitorState.running,
  command: "pnpm --filter codex-agents-web run typecheck:fast",
  display: {
    kind: BackgroundTaskKind.monitor,
    kindLabel: BackgroundTaskKindLabel.monitor,
    title: "pnpm --filter codex-agents-web run typecheck:fast",
    active: true,
    indicator: BackgroundTaskIndicator.working,
    label: BackgroundTaskLabel.running,
    inBackgroundList: true,
    actions: ["readOutput"],
  },
};
const meta = {
  title: "Agents/Background activity",
  component: BackgroundTaskStrip,
  args: {
    task,
    now: 1700000195,
    selected: false,
    select: fn(),
    owner: { id: "worker", name: "Release reviewer" },
  },
  decorators: [
    (Story) => (
      <div
        className="tasks-drawer"
        style={{ width: "min(var(--studio-team-size), 100%)" }}
      >
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof BackgroundTaskStrip>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Monitor: Story = {
  play: async ({ canvasElement, args }) => {
    await userEvent.click(within(canvasElement).getByRole("button"));
    await expect(args.select).toHaveBeenCalledOnce();
  },
};
export const Approval: Story = {
  args: {
    task: {
      ...task,
      status: "approval",
      display: {
        ...task.display,
        label: BackgroundTaskLabel.approval,
        indicator: BackgroundTaskIndicator.answer,
        actions: ["readOutput", "approve"],
      },
    },
  },
};
export const Tool: Story = {
  args: {
    task: {
      ...task,
      kind: "tool",
      command: null,
      name: "webSearch",
      display: {
        ...task.display,
        kind: BackgroundTaskKind.tool,
        kindLabel: BackgroundTaskKindLabel.tool,
        title: "Web search",
      },
    },
  },
};
export const UnknownOutcome: Story = {
  args: {
    task: {
      ...task,
      status: "lost",
      display: {
        ...task.display,
        active: false,
        indicator: BackgroundTaskIndicator.error,
        label: BackgroundTaskLabel.lost,
      },
    },
  },
};

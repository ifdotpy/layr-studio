import { useBackgroundTaskArea } from "../support/backgroundTaskActions.tsx";
import {
  BackgroundTaskIndicator,
  BackgroundTaskKind,
  BackgroundTaskKindLabel,
  BackgroundTaskLabel,
  MonitorState,
  ProcessSourceKind,
} from "../../../../generated/api.ts";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { useState } from "react";
import { ServerServicesProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createServerServices } from "../../../../servers/serverServices.ts";
import type { ReactNode } from "react";
import {
  createStateMirror,
  type ObservableResources,
} from "../../../../generated/observableResources.ts";
import { agentSummaryFixture } from "../../../../servers/agentSummaryFixture.ts";
import { BackgroundTaskActionsProvider } from "../support/backgroundTaskActions.tsx";
import { BackgroundTaskStrip } from "../BackgroundTaskStrip/BackgroundTaskStrip.tsx";
import type { components } from "../../../../generated/api.ts";
const task: components["schemas"]["ProcessView"] = {
  id: "monitor",
  key: "monitor-key",
  generation: "gen",
  accountKey: "default",
  epoch: 1,
  source: ProcessSourceKind.monitor,
  name: "Command monitor",
  bytes: 0,
  cancelRequested: false,
  outputTruncated: false,
  outputPatterns: [],
  tail: "",
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
function StoryArea({ children }: { children: ReactNode }) {
  const actions = useBackgroundTaskArea({
    opened: true,
    leadId: "worker",
    close() {},
    openAgent() {},
    notify() {},
  });
  return (
    <BackgroundTaskActionsProvider value={actions}>
      {children}
    </BackgroundTaskActionsProvider>
  );
}
const meta = {
  title: "Agents/Background activity",
  component: BackgroundTaskStrip,
  args: {
    task,
  },
  decorators: [
    (Story, context) => {
      const [services] = useState(() => {
        const state = createStateMirror();
        state.apply([
          {
            collection: "agentSummary",
            id: "worker",
            value: agentSummaryFixture("worker", { name: "Release reviewer" }),
          },
        ]);
        const model = context.args.task;
        const services = createServerServices({
          id: "story",
          label: "Story",
          origin: "http://127.0.0.1:4620",
        });
        const store: ObservableResources = {
          ...services.store,
          state: () => ({ current: { status: "live", data: state.current } }),
          requests: () => ({ current: { status: "loading" } }),
          questionHistory: () => ({ current: { status: "loading" } }),
          processes: () => ({
            current: {
              status: "live",
              data: {
                processes: [],
                activities: [
                  "key" in model
                    ? { source: "process", value: model }
                    : { source: "task", value: model },
                ],
              },
            },
          }),
          monitors: () => ({ current: { status: "live", data: [] } }),
          agentDetail: () => ({ current: { status: "loading" } }),
          taskDetail: () => ({ current: { status: "loading" } }),
          transcript: () => ({ current: { status: "loading" } }),
        };
        Object.defineProperty(services, "store", {
          value: { ...store, dispose() {} },
        });
        return services;
      });
      return (
        <ServerServicesProvider services={services}>
          <StoryArea>
            <div
              className="tasks-drawer"
              style={{ width: "min(var(--studio-team-size), 100%)" }}
            >
              <Story />
            </div>
          </StoryArea>
        </ServerServicesProvider>
      );
    },
  ],
} satisfies Meta<typeof BackgroundTaskStrip>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Monitor: Story = {
  play: async ({ canvasElement }) => {
    await userEvent.click(within(canvasElement).getByRole("button"));
    await expect(within(canvasElement).getByRole("button")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
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
      id: "tool",
      agent: "worker",
      created: 1700000000,
      status: "running",
      turnId: "turn",
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

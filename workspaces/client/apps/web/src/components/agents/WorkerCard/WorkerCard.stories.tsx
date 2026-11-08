import { AgentSource, AgentStatus, AgentRole } from "../../../generated/api.ts";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo } from "react";
import { expect, fn, userEvent, within } from "storybook/test";
import WorkerCard from "./WorkerCard.tsx";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import { TeamProvider } from "../support/TeamPanelContext.tsx";
import { useTeamView } from "../support/state.ts";
import type { SidebarAgent } from "../../sidebar/support/viewModel.ts";
import type { Agent } from "../../../types.ts";

const agent: Agent = {
  id: "worker-1",
  rootId: "lead",
  parentId: "lead",
  name: "Transcript worker",
  source: AgentSource["managed"],
  status: AgentStatus["running"],
  operationalStatus: { reason: "working" },
  model: "gpt-6",
  created: 1,
  role: AgentRole["worker"],
  inFlight: true,
  overview: {
    task: "Extract reusable transcript presentation components.",
    result: "The story renders the same typed card used by the team panel.",
  },
};
const actions = { open: fn(), previewResult: fn(), remove: fn() };
const noop = () => {};
function Card({ agent, selected }: { agent: SidebarAgent; selected: boolean }) {
  const view = useTeamView("lead");
  return (
    <TeamProvider
      value={{
        ...actions,
        view,
        retainedWorktrees: () => [],
        hasTeam: () => true,
        opened: selected ? agent.id : null,
        workerQuery: "",
        workerFilter: "all",
        completedOpen: false,
        setWorkerQuery: noop,
        setWorkerFilter: noop,
        setCompletedOpen: noop,
        setTeamOpen: noop,
        setWideTeamOpen: noop,
      }}
    >
      <WorkerCard agent={agent} />
    </TeamProvider>
  );
}
function Fixture({
  selected = false,
  needsAnswer = false,
}: {
  selected?: boolean;
  needsAnswer?: boolean;
}) {
  const fixture = useMemo(
    () =>
      observableFixture([
        {
          ...agent,
          operationalStatus: needsAnswer
            ? { reason: "needs_input", wait: null }
            : { reason: "working" },
        },
      ]),
    [needsAnswer],
  );
  const model = fixture.mirror.current.agents.get(agent.id)!;
  return (
    <fixture.Provider>
      <Card agent={model} selected={selected} />
    </fixture.Provider>
  );
}
const meta = { title: "Agents/Worker card", component: Fixture } satisfies Meta<
  typeof Fixture
>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Working: Story = {
  play: async ({ canvasElement }) => {
    await userEvent.click(
      within(canvasElement).getByRole("button", { name: /Transcript worker/ }),
    );
    await expect(actions.open).toHaveBeenCalledOnce();
  },
};
export const SelectedNeedsAnswer: Story = {
  args: { selected: true, needsAnswer: true },
};

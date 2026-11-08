import { observer } from "mobx-react-lite";
import type { CSSProperties } from "react";
import { UnstyledButton } from "@mantine/core";
import type { SidebarAgent } from "../../sidebar/support/viewModel.ts";

import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
import { useTeam } from "../support/TeamPanelContext.tsx";
import { workerDepth } from "../support/teamModel.ts";
import { WorkerHeading } from "../worker/WorkerHeading/WorkerHeading.tsx";
import { WorkerActivity } from "../worker/WorkerActivity/WorkerActivity.tsx";
import { WorkerMetadata } from "../worker/WorkerMetadata/WorkerMetadata.tsx";
import { WorkerDetails } from "../worker/WorkerDetails/WorkerDetails.tsx";
import { WorkerOptions } from "../worker/WorkerOptions/WorkerOptions.tsx";
import { WorkerResult } from "../worker/WorkerResult/WorkerResult.tsx";
import { WorkerRetainedWorktree } from "../worker/WorkerRetainedWorktree/WorkerRetainedWorktree.tsx";

export type WorkerProps = { agent: SidebarAgent };

export default observer(function WorkerCard({ agent }: WorkerProps) {
  reportPromptComposerRender("team-row", agent.id);
  const { view, opened, open } = useTeam();
  const selected = opened === agent.id;
  const depth = workerDepth(agent, view.byId, view.lead?.id);
  return (
    <div
      className={`worker-entry ${selected ? "selected" : ""}`}
      data-studio-state={view.indicator(agent).kind}
      data-has-parent={depth > 0 || undefined}
      style={{ "--worker-depth": depth } as CSSProperties}
    >
      <div className="worker-heading">
        <UnstyledButton
          className="worker"
          data-worker={agent.id}
          aria-current={selected ? "page" : undefined}
          onClick={() => open(agent.id)}
        >
          <span className="worker-text">
            <WorkerHeading agent={agent} />
            <WorkerActivity agent={agent} />
          </span>
        </UnstyledButton>
        <WorkerOptions agent={agent} />
      </div>
      <WorkerRetainedWorktree agent={agent} />
      <div className="worker-bottom">
        <WorkerMetadata agent={agent} />
        <WorkerDetails agent={agent} />
      </div>
      <WorkerResult agent={agent} />
    </div>
  );
});

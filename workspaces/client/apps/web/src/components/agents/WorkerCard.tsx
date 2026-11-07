import { observer } from "mobx-react-lite";
import type { CSSProperties } from "react";
import { UnstyledButton } from "@mantine/core";
import type { SidebarAgent } from "../sidebar/viewModel.ts";

import { reportPromptComposerRender } from "../prompt-composer/renderProbe";
import { useTeam } from "./TeamPanelContext.tsx";
import { workerDepth } from "./teamModel.ts";
import { WorkerHeading } from "./worker/Heading.tsx";
import { WorkerActivity } from "./worker/Activity.tsx";
import { WorkerMetadata } from "./worker/Metadata.tsx";
import { WorkerDetails } from "./worker/Details.tsx";
import { WorkerOptions } from "./worker/Options.tsx";
import { WorkerResult } from "./worker/Result.tsx";
import { WorkerRetainedWorktree } from "./worker/Retained.tsx";

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

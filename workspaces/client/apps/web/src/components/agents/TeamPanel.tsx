import { useServerApi } from "../../servers/ServerServicesContext.tsx";
import { observer } from "mobx-react-lite";
import { useAgentDetail } from "./state.ts";
import { useTeam } from "./TeamPanelContext.tsx";
import type { SidebarAgent } from "../sidebar/viewModel.ts";
import { TeamRetainedWorktrees } from "./RetainedWorktrees";
import { ActionIcon, Button, TextInput } from "@mantine/core";
import { ArrowLeft, Search, X } from "lucide-react";
import UIErrorBoundary from "../UIErrorBoundary";
import WorkerCard from "./WorkerCard";
import Disclosure from "../ui/Disclosure";
import { TeamModels, TeamSummary, TeamGroupLabel } from "./WorkerOverview";

export const TeamPanelView = observer(function TeamPanelView() {
  const {
    view,
    open,
    retainedWorktrees,
    opened,
    workerQuery,
    setWorkerQuery,
    completedOpen,
    setCompletedOpen,
    setTeamOpen,
    setWideTeamOpen,
  } = useTeam();
  const { save } = useServerApi();
  const lead = view.lead;
  const query = view.query;
  const shown = view.shown;
  const { groups, completed } = view.dispatch;
  const worker = (agent: SidebarAgent) => (
    <UIErrorBoundary key={agent.id} label="this subagent" resetKey={agent.id}>
      <WorkerCard agent={agent} />
    </UIErrorBoundary>
  );
  return (
    <TeamModels>
      <aside
        id="team"
        aria-label="Team"
        className={`team-dispatch ${view.smallTeam ? "team-compact" : ""}`}
      >
        <div className="team-heading">
          <h2>Team</h2>
          <ActionIcon
            aria-label="Close team"
            id="team-close"
            onClick={() => {
              setTeamOpen(false);
              setWideTeamOpen(false);
              save("codex-team-open", false);
            }}
          >
            <X size={16} />
          </ActionIcon>
        </div>
        {lead && <LeadReport agent={lead} />}
        <TeamRetainedWorktrees items={retainedWorktrees} />
        <TeamSummary />
        {/* The lead link is only useful from a worker chat. */}
        {lead && opened !== lead.id && (
          <Button
            id="lead-row"
            variant="subtle"
            size="compact-sm"
            title={lead.name || "Main agent"}
            leftSection={<ArrowLeft size={13} />}
            onClick={() => open(lead.id)}
          >
            Back to main agent
          </Button>
        )}
        {view.showFilters && (
          <>
            <TextInput
              leftSection={<Search size={14} />}
              id="worker-search"
              type="search"
              aria-label="Find a subagent"
              placeholder="Find a worker"
              value={workerQuery}
              onChange={(e) => setWorkerQuery(e.target.value)}
            />
            {!!query && (
              <p className="team-search-count" role="status">
                {shown.length} {shown.length === 1 ? "match" : "matches"} in
                this team
              </p>
            )}
          </>
        )}
        <div id="workers">
          {groups.map((group) =>
            group.workers.length ? (
              <section
                className="team-status-group"
                aria-label={group.name}
                key={group.name}
              >
                <h3>
                  <TeamGroupLabel
                    kind={group.kind}
                    label={group.name}
                    count={group.workers.length}
                  />
                </h3>
                {group.workers.map(worker)}
              </section>
            ) : null,
          )}
          {!!completed.length && (
            <Disclosure
              className="worker-group"
              open={!!query || completedOpen}
              onChange={(open) => {
                if (!query) setCompletedOpen(open);
              }}
              ariaLabel={`Finished (${completed.length})`}
              chevron
              summary={
                <TeamGroupLabel
                  kind="completed"
                  label="Finished"
                  count={completed.length}
                />
              }
            >
              {/* A collapsed group keeps its rows out of the DOM; the control
                  and the team overview still count them. */}
              {(!!query || completedOpen) && completed.map(worker)}
            </Disclosure>
          )}
          {!shown.length && (
            <p className="team-empty" role="status">
              {query
                ? "No workers match your search."
                : "No workers in this group."}
            </p>
          )}
        </div>
      </aside>
    </TeamModels>
  );
});

const LeadReport = observer(function LeadReport({
  agent,
}: {
  agent: SidebarAgent;
}) {
  const detail = useAgentDetail(agent);
  const { open, previewResult } = useTeam();
  const lead = agent;
  return (
    <>
      {detail && (detail.overview?.result || detail.overview?.resultFile) && (
        <Disclosure
          className="team-lead-report"
          summary="Main agent report"
          chevron
        >
          {detail.overview.result && <p>{detail.overview.result}</p>}
          {detail.overview.resultTruncated && (
            <Button
              variant="subtle"
              size="compact-xs"
              onClick={() => open(lead.id)}
            >
              Continue in main chat
            </Button>
          )}
          {detail.overview.resultFile && (
            <Button
              component="a"
              href={detail.overview.resultFile}
              variant="subtle"
              size="compact-xs"
              onClick={(event) => {
                event.preventDefault();
                previewResult({
                  agent: lead.id,
                  path: detail.overview?.resultFile ?? undefined,
                });
              }}
            >
              Open main agent submitted result
            </Button>
          )}
        </Disclosure>
      )}
    </>
  );
});

export default TeamPanelView;

import {
  Button,
  Loader,
  NativeSelect,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import {
  BookOpen,
  ChevronRight,
  Clock3,
  FileDiff,
  GitBranch,
  Inbox,
  Layers3,
  RefreshCw,
  Search,
  Users,
  Wrench,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { useEffect, useRef, useState } from "react";
import type { ServerApi } from "../../../api.ts";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import type { Agent } from "../../../types.ts";
import FilePreview, {
  type PreviewTarget,
} from "../../conversation/FilePreview/FilePreview.tsx";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { SidebarAgent } from "../../sidebar/support/viewModel.ts";
import { PanelHeader } from "../../ui/PanelHeader/PanelHeader.tsx";
import TeamChats from "../messages/TeamChats/TeamChats.tsx";
import { SearchContent } from "../search/SearchContent/SearchContent.tsx";
import Tasks from "../tasks/Tasks/Tasks.tsx";
import {
  WorkspacePanelHost,
  type WorkspacePanelPresentationProps,
} from "../WorkspacePanelHost/WorkspacePanelHost.tsx";
import { WorkspaceSection as Empty } from "../WorkspaceSection/WorkspaceSection.tsx";
import { WorkspaceChanges as Changes } from "../WorkspaceChanges/WorkspaceChanges.tsx";
import { WorkspaceCheckpoints as Checkpoints } from "../WorkspaceCheckpoints/WorkspaceCheckpoints.tsx";
import { WorkspacePlan as Plan } from "../WorkspacePlan/WorkspacePlan.tsx";
import { WorkspaceProfiles as Profiles } from "../WorkspaceProfiles/WorkspaceProfiles.tsx";
import { WorkspaceRules as Rules } from "../WorkspaceRules/WorkspaceRules.tsx";
import { WorkspaceTools as Tools } from "../WorkspaceTools/WorkspaceTools.tsx";
import { WorkspaceStore } from "../../../app/shell/WorkspaceStore.ts";

export type Props = WorkspacePanelPresentationProps & {
  initialSection?: string;
  initialFocus?: { id: string; requestId: string; roomId?: string } | undefined;
  opened: boolean;
  onClose: () => void;
  agent?: Agent | undefined;
  onSelect: (id: string, messageId?: string) => void;
  notify: (s: string) => void;
};
export type Context = Props & {
  selected: SidebarAgent | undefined;
  run: ServerApi["post"];
  reload: () => void;
  preview: (target: PreviewTarget) => void;
  navigate: (section: string, agent?: string, item?: string) => void;
  model: WorkspaceStore;
};
const sections = [
  ["changes", "Changes", FileDiff],
  ["messages", "Messages", Inbox],
  ["search", "Search", Search],
  ["plan", "Plan", BookOpen],
  ["tasks", "Tasks", Layers3],
  ["checkpoints", "Checkpoints", GitBranch],
  ["tools", "Tools", Wrench],
  ["profiles", "Profiles", Users],
  ["rules", "Rules", Clock3],
] as const;
const descriptions: Record<string, string> = {
  changes: "Latest changes by this agent.",
  messages: "Your messages and this team’s conversations.",
  search: "Find messages, work, plans, and agent conversations.",
  plan: "The agent's current plan.",
  tasks: "Task assignments, checks, results, and review decisions.",
  checkpoints: "Save and restore points in the work.",
  tools: "Tools the agents can use.",
  profiles: "Reusable instructions and model choices for workers.",
  rules: "When the agent resumes",
};
export const Workspace = observer(function Workspace(props: Props) {
  const store = useServerStore();
  const areas = useServerAreas();
  const workspace = areas.workspace;
  const [model] = useState(
    () => new WorkspaceStore(workspace, store, props, areas),
  );
  // integration: AppUi workspace entry actions should call model.enter directly.
  useEffect(
    () => model.enter(props),
    [model, props.opened, props.agent?.id, props.initialSection],
  );
  const { section, agentId, managedAgents, selected, filePath, attention } =
    model;
  const { preview } = model;
  const setPreview = model.previewFile;
  const navRef = useRef<HTMLElement>(null);
  useEffect(() => {
    if (props.opened)
      navRef.current
        ?.querySelector('[aria-current="page"]')
        ?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [section, props.opened]);
  const { reload } = workspace;
  const openSelected = () => model.openSelected(props);
  const context: Context = {
    ...props,
    model,
    selected,
    run: workspace.api.post,
    reload,
    preview: setPreview,
    navigate: model.navigate,
  };
  const needAgent = [
    "changes",
    "plan",
    "tasks",
    "checkpoints",
    "tools",
    "rules",
  ].includes(section);
  const title = sections.find(([id]) => id === section)?.[1];
  return (
    <WorkspacePanelHost
      {...props}
      resizeScope={`${props.resizeScope || ""}:${props.agent?.id || ""}`}
      drawerSize={
        section === "changes" || section === "messages"
          ? "min(var(--layout-workspace-drawer-wide-max), 100vw)"
          : "min(var(--layout-workspace-drawer-default-max), 100vw)"
      }
      className={section === "messages" ? "workspace-messages-drawer" : ""}
      title={
        <>
          {section === "messages" ? <Inbox size={19} /> : <Layers3 size={19} />}
          {section === "messages" ? "Messages" : "Workspace"}
        </>
      }
      actions={
        <>
          {section === "messages" && (
            <Button
              variant="subtle"
              size="compact-sm"
              className="workspace-messages-refresh"
              aria-label="Refresh messages"
              onClick={reload}
            >
              <RefreshCw size={16} />
            </Button>
          )}
        </>
      }
    >
      <div
        className={`workspace-shell ${section === "messages" ? "workspace-focused-messages" : ""}`}
      >
        {section !== "messages" && (
          <nav
            ref={navRef}
            className="workspace-nav"
            aria-label="Workspace sections"
          >
            {sections.map(([id, label, Icon]) => (
              <UnstyledButton
                key={id}
                className={`workspace-nav-item ${section === id ? "selected" : ""}`}
                onClick={() => model.navigate(id)}
                aria-current={section === id ? "page" : undefined}
              >
                <Icon size={17} />
                <span>{label}</span>
                {id === "messages" && attention > 0 && (
                  <span className="workspace-count">{attention}</span>
                )}
              </UnstyledButton>
            ))}
          </nav>
        )}
        <main
          className={`workspace-content ${section === "messages" ? "workspace-messages" : ""}`}
        >
          {section !== "messages" && (
            <div className="workspace-heading">
              <div>
                <PanelHeader title={title} help={descriptions[section]} />
                {section !== "messages" && section !== "profiles" && (
                  <div className="workspace-scope">
                    <NativeSelect
                      aria-label="Agent"
                      size="xs"
                      value={agentId}
                      onChange={(e) => model.select(e.target.value)}
                      data={[
                        { value: "", label: "Select an agent" },
                        ...managedAgents.map((a) => {
                          const parent = managedAgents.find(
                            (candidate) => candidate.id === a.parentId,
                          );
                          const prefix = a.isLead
                            ? "Main agent · "
                            : parent
                              ? `↳ ${parent.name} · `
                              : "";
                          return { value: a.id, label: prefix + a.name };
                        }),
                      ]}
                    />
                    {selected && (
                      <Button
                        variant="subtle"
                        size="compact-xs"
                        onClick={openSelected}
                      >
                        Open chat <ChevronRight size={14} />
                      </Button>
                    )}
                    {[...model.submittingPanels].some(
                      (panel) => panel.busy,
                    ) && (
                      <Loader
                        size={16}
                        aria-label="Workspace operation in progress"
                      />
                    )}
                  </div>
                )}
              </div>
              {section === "plan" && (
                <Button
                  variant="subtle"
                  size="compact-sm"
                  disabled={!selected}
                  onClick={openSelected}
                >
                  Change plan in chat
                </Button>
              )}
              {section === "changes" && selected && (
                <TextInput
                  aria-label="Open a file"
                  placeholder="File path"
                  value={filePath}
                  onChange={(e) => model.setFilePath(e.target.value)}
                  rightSection={
                    <UnstyledButton
                      aria-label="Preview file"
                      disabled={!filePath.trim()}
                      onClick={() =>
                        setPreview({ agent: selected.id, path: filePath })
                      }
                    >
                      <ChevronRight size={16} />
                    </UnstyledButton>
                  }
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && filePath.trim())
                      setPreview({ agent: selected.id, path: filePath });
                  }}
                />
              )}
              <Button
                variant="subtle"
                size="compact-sm"
                aria-label="Refresh workspace"
                onClick={reload}
              >
                <RefreshCw size={16} />
              </Button>
            </div>
          )}
          {props.opened &&
            (needAgent && !selected ? (
              <Empty>Select an agent to view its {title?.toLowerCase()}.</Empty>
            ) : (
              <div
                className={
                  section === "messages" ? "workspace-message-body" : undefined
                }
                key={`${section}:${section === "messages" ? "team" : selected?.id || "all"}`}
              >
                {section === "changes" && <Changes {...context} />}
                {section === "messages" && <TeamChats />}
                {section === "search" && (
                  <SearchContent
                    source={{
                      opened: props.opened,
                      close: props.onClose,
                      select: props.onSelect,
                      navigate: context.navigate,
                    }}
                  />
                )}
                {section === "plan" && <Plan {...context} />}
                {section === "tasks" && selected && (
                  <Tasks
                    key={selected.id}
                    agentId={selected.id}
                    rootId={selected.rootId || selected.id}
                  />
                )}
                {section === "checkpoints" && <Checkpoints {...context} />}
                {section === "tools" && <Tools {...context} />}
                {section === "profiles" && <Profiles {...context} />}
                {section === "rules" && <Rules {...context} />}
              </div>
            ))}
        </main>
      </div>
      <FilePreview
        target={props.opened ? preview : null}
        onClose={() => setPreview(null)}
      />
    </WorkspacePanelHost>
  );
});
export default Workspace;

export { WorkspaceResources } from "../../../app/shell/WorkspaceResources.ts";
export {
  parseRuleDraft,
  ruleBodyFromDraft,
  ruleDraftFromRecord,
} from "../../../app/shell/WorkspaceRules.ts";

import { Drawer } from "@mantine/core";
import TeamPanel from "../../agents/TeamPanelView/TeamPanelView.tsx";
import { TeamPresence } from "../../agents/TeamPresence/TeamPresence.tsx";
import Workspace from "../Workspace/Workspace.tsx";
import BackgroundTasks from "../tasks/BackgroundTasks/BackgroundTasks.tsx";
import TerminalDock from "../TerminalDock/TerminalDock.tsx";
import FilePreview from "../../conversation/FilePreview/FilePreview.tsx";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const AppPanels = observer(function AppPanels() {
  const a = useServerAreas(),
    active = a.scope.screen.active,
    lead = a.navigation.selectedLead,
    agent = a.navigation.selectedAgent;
  return (
    <>
      {a.layout.isNarrowTeam ? (
        <TeamPresence>
          <Drawer
            opened={a.ui.isTeamVisible}
            onClose={a.ui.closeTeamDrawer}
            position="right"
            size="var(--studio-team-size)"
            padding={0}
            withCloseButton={false}
            title="Team"
            classNames={{ header: "sr-only" }}
          >
            <TeamPanel />
          </Drawer>
        </TeamPresence>
      ) : a.ui.isTeamVisible ? (
        <TeamPanel />
      ) : null}
      {a.ui.visited && a.startup.isReady && (
        <Workspace
          key={`workspace:${lead?.id || "none"}`}
          opened={active && a.ui.opened}
          onClose={a.ui.closeWorkspace}
          initialSection={a.ui.section}
          initialFocus={a.ui.focus}
          inlineTarget={a.layout.slot}
          panelLayout={a.layout.panelLayout}
          onPanelWidthChange={a.layout.setPreferredPanelWidth}
          resizeScope={
            a.startup.workspaceId || a.startup.creationScope || "pending"
          }
          agent={agent || lead}
          onSelect={a.open}
          notify={a.notices.notify}
        />
      )}
      <BackgroundTasks
        actions={a.ui.activity}
        key={`tasks:${lead?.id || "none"}`}
        opened={active && a.ui.tasksOpen}
        initialFocus={a.ui.taskFocus}
        close={a.ui.closeTasks}
        afterClose={a.ui.afterTaskClose}
        leadId={active && a.ui.tasksOpen ? lead?.id : undefined}
        openAgent={a.open}
        notify={a.notices.notify}
      />
      {a.ui.terminalVisible && <TerminalDock />}
      <FilePreview
        target={active ? a.ui.filePreview : null}
        onClose={a.ui.closePreview}
      />
    </>
  );
});

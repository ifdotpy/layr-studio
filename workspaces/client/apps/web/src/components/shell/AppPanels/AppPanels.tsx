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
    ui = a.ui,
    l = a.layout,
    active = a.scope.screen.active,
    lead = a.navigation.selectedLead,
    agent = a.navigation.selectedAgent;
  return (
    <>
      {a.layout.isNarrowTeam ? (
        <TeamPresence>
          <Drawer
            opened={ui.isTeamVisible}
            onClose={ui.closeTeamDrawer}
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
      ) : ui.isTeamVisible ? (
        <TeamPanel />
      ) : null}
      {ui.visited && a.startup.isReady && (
        <Workspace
          key={`workspace:${lead?.id || "none"}`}
          opened={active && ui.opened}
          onClose={ui.closeWorkspace}
          initialSection={ui.section}
          initialFocus={ui.focus}
          inlineTarget={l.slot}
          panelLayout={l.panelLayout}
          onPanelWidthChange={l.setPreferredPanelWidth}
          resizeScope={
            a.startup.workspaceId || a.startup.creationScope || "pending"
          }
          agent={agent || lead}
          onSelect={a.open}
          notify={a.notices.notify}
        />
      )}
      <BackgroundTasks
        key={`tasks:${lead?.id || "none"}`}
        opened={active && ui.tasksOpen}
        initialFocus={ui.taskFocus}
        close={ui.closeTasks}
        afterClose={ui.afterTaskClose}
        leadId={active && ui.tasksOpen ? lead?.id : undefined}
        openAgent={a.open}
        notify={a.notices.notify}
      />
      {active && !l.isMobile && a.preferences.value.showTerminals && (
        <TerminalDock agent={agent || lead} notify={a.notices.notify} />
      )}
      <FilePreview
        target={active ? ui.filePreview : null}
        onClose={ui.closePreview}
      />
    </>
  );
});

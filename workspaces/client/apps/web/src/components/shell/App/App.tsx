import { AppTeam } from "../AppTeam/AppTeam.tsx";
import { AppPanels } from "../AppPanels/AppPanels.tsx";
import { AppDialogs } from "../AppDialogs/AppDialogs.tsx";
import { useAppPreferences } from "../../../app/settings/useAppPreferences.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import Sidebar from "../../sidebar/Sidebar/Sidebar.tsx";
import { ChatActionsMenu } from "../../conversation/ChatActionsMenu/ChatActionsMenu.tsx";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
import {
  StartupStatus,
  SchemaUpdateDialog,
  ServerUpdateStatus,
} from "../ServerUpdateStatus/ServerUpdateStatus.tsx";
import { LayoutHost } from "../LayoutHost/LayoutHost.tsx";
import { WorkspaceSlot } from "../WorkspaceSlot/WorkspaceSlot.tsx";
import { RailRoot } from "../RailRoot/RailRoot.tsx";
import { ConversationHeader } from "../../conversation/ConversationHeader/ConversationHeader.tsx";
import { ManagedConversationRoot } from "../../conversation/ManagedConversationRoot/ManagedConversationRoot.tsx";
import SearchOverlay from "../search/SearchOverlay/SearchOverlay.tsx";
import SidebarWarnings from "../../sidebar/SidebarWarnings/SidebarWarnings.tsx";
import SupervisorRecoveryNotice from "../SupervisorRecoveryNotice/SupervisorRecoveryNotice.tsx";
import { AppSettings } from "../AppSettings/AppSettings.tsx";
import { AppNotices } from "../AppNotices/AppNotices.tsx";
import { AppAccountNotices } from "../AppAccountNotices/AppAccountNotices.tsx";
import { ToastNotice } from "../ToastNotice/ToastNotice.tsx";
export { AppUi } from "../../../app/shell/AppUi.tsx";
export { ToastNotice } from "../ToastNotice/ToastNotice.tsx";
export default observer(function App() {
  const a = useServerAreas(),
    { scope, startup, layout } = a;
  useAppPreferences(a);
  reportPromptComposerRender("app");
  if (!scope.screen.active) return null;
  if (scope.screen.channel && startup.isSchemaGateVisible)
    return <ServerUpdateStatus />;
  if (!startup.isReady || !a.state)
    return startup.isSchemaGateVisible ? (
      <SchemaUpdateDialog />
    ) : (
      <StartupStatus />
    );
  return (
    <AppTeam>
      <div ref={layout.attachHost} style={{ display: "contents" }}>
        <AppSidebar />
        <SidebarWarnings />
        <RailRoot menu={<ChatActionsMenu />} />
        <LayoutHost>
          <AppWorkspace />
          <WorkspaceSlot />
          <AppPanels />
        </LayoutHost>
        <AppSearch />
        <AppSettings />
        <AppDialogs />
        <SchemaUpdateDialog />
        <ToastNotice />
      </div>
    </AppTeam>
  );
});

const AppWorkspace = observer(function AppWorkspace() {
  const { layout, preferences } = useServerAreas();
  return (
    <main
      ref={layout.attachWorkspace}
      className="workspace"
      data-edge-rail={layout.railGeometry ? "true" : undefined}
      data-show-message-avatars={preferences.value.showMessageAvatars}
    >
      <SupervisorRecoveryNotice />
      {!layout.railGeometry && <ConversationHeader />}
      <AppNotices />
      <AppAccountNotices />
      <ManagedConversationRoot />
    </main>
  );
});

const AppSidebar = observer(function AppSidebar() {
  const a = useServerAreas();
  return a.sidebar.delegated || !a.ui.sidebarList ? null : (
    <Sidebar owner={a.ui.sidebarList} />
  );
});

const AppSearch = observer(function AppSearch() {
  const a = useServerAreas();
  return (
    <SearchOverlay
      source={{
        opened: a.ui.searchOpen,
        close: a.ui.closeSearch,
        select: (id, messageId) =>
          a.navigation.selectExternal(id, {
            requestId: randomUUID(),
            ...(messageId ? { messageId } : {}),
          }),
        navigate: (section, agent) => {
          if (agent) a.open(agent);
          a.ui.showSection(section);
        },
      }}
    />
  );
});

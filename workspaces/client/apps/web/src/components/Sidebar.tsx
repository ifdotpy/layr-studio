import { reportPromptComposerRender } from "./prompt-composer/renderProbe.ts";
import { useLayoutEffect } from "react";
import { observer } from "mobx-react-lite";
import { SidebarRowActionsProvider } from "./chat-row/actions.ts";
import "./sidebar-projects.css";
import { SidebarBackendProvider } from "./sidebar/services";

import { PeerTeamActionsProvider } from "./shell/messages/PeerTeams";
import { SidebarContents } from "./sidebar/SidebarContents.tsx";
import { SidebarStateProvider } from "./sidebar/SidebarState.tsx";
import type { SidebarListOwner } from "../app/shell/SidebarOwner.ts";
export default observer(function Sidebar({
  owner,
  notice,
}: {
  owner: SidebarListOwner;
  notice?: string;
}) {
  useLayoutEffect(() => owner.attachPresentation(), [owner]);
  reportPromptComposerRender("sidebar");
  return (
    <SidebarBackendProvider value={owner.services.cache}>
      <SidebarStateProvider value={owner}>
        <SidebarRowActionsProvider value={owner.actions.rows}>
          <PeerTeamActionsProvider value={owner.actions.teams}>
            <SidebarContents notice={notice} />
          </PeerTeamActionsProvider>
        </SidebarRowActionsProvider>
      </SidebarStateProvider>
    </SidebarBackendProvider>
  );
});

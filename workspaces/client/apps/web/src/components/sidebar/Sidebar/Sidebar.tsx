import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
import { useLayoutEffect } from "react";
import { observer } from "mobx-react-lite";
import { SidebarRowActionsProvider } from "../chat-row/support/actions.ts";

import { SidebarBackendProvider } from "../support/services.tsx";

import { PeerTeamActionsProvider } from "../../shell/messages/PeerTeamGroup/PeerTeamGroup.tsx";
import { SidebarContents } from "../SidebarContents/SidebarContents.tsx";
import { SidebarStateProvider } from "../support/SidebarState.tsx";
import type { SidebarListOwner } from "../../../app/shell/SidebarOwner.ts";
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

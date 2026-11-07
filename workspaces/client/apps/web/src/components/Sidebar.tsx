import { observer } from "mobx-react-lite";
import type { DeepReadonly } from "../deepReadonly.ts";
import { type StateCollections } from "../generated/observableResources.ts";
import { type Snapshot } from "../types";
import { SidebarRowActionsProvider } from "./chat-row/actions.ts";
import "./sidebar-projects.css";
import {
  localSidebarServices,
  SidebarBackendProvider,
  type SidebarServices,
} from "./sidebar/services";
import {
  type SidebarAgent,
  type SidebarChatView,
} from "./sidebar/viewModel.ts";

import { type RemoteProjectList } from "./sidebar/RemoteProjects";

export type SidebarProps = {
  services?: SidebarServices;
  notice?: string;
  data: DeepReadonly<Snapshot>;
  rows?: readonly SidebarChatView[];
  collections?: StateCollections;
  /** Registered project paths when data also contains inferred view projects. */
  registeredProjectPaths?: ReadonlySet<string>;
  opened: string | null;
  selectionIdentity?: string;
  /** Projects of paired servers, listed read-only below this server's projects. */
  remoteProjects?: RemoteProjectList;
  creating: boolean;
  mobile: boolean;
  collapsed?: boolean;
  onCompactChange?: (compact: boolean) => void;
  close: () => void;
  offlineServers?: ReadonlySet<string>;
  markingRead: Set<string>;
  pendingReadTargets?: ReadonlyMap<
    string,
    NonNullable<SidebarAgent["markRead"]>
  >;
  hideOldChatsThreshold?: number;
};
import { PeerTeamActionsProvider } from "./shell/messages/PeerTeams";
import { useSidebarActions } from "./sidebar/sidebarActions.ts";
import { SidebarContents } from "./sidebar/SidebarContents.tsx";
import {
  SidebarStateProvider,
  useSidebarController,
} from "./sidebar/SidebarState.tsx";
export default function Sidebar(p: SidebarProps) {
  const services = p.services || localSidebarServices;
  return (
    <SidebarBackendProvider value={services.cache}>
      <SidebarOwner {...p} services={services} />
    </SidebarBackendProvider>
  );
}
const SidebarOwner = observer(function SidebarOwner(
  p: SidebarProps & { services: SidebarServices },
) {
  const state = useSidebarController(p);
  const actions = useSidebarActions(state);
  return (
    <SidebarStateProvider value={state}>
      <SidebarRowActionsProvider value={actions.rows}>
        <PeerTeamActionsProvider value={actions.teams}>
          <SidebarContents />
        </PeerTeamActionsProvider>
      </SidebarRowActionsProvider>
    </SidebarStateProvider>
  );
});

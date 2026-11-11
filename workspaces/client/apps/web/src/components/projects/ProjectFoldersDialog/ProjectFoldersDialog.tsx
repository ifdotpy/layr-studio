import type { createProjectPeersStore } from "../ProjectPeers/ProjectPeers.tsx";
import type { ProjectModel } from "../../../app/contracts.ts";
import { Tabs } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../store/runtime.ts";
import {
  discoveryProjectServers,
  type LocationProjectView,
} from "../../../servers/projectLocations.ts";
import type { DirectoryBrowse } from "../ProjectDirectoryPicker/ProjectDirectoryPicker.tsx";
import ProjectLocationsSettings, {
  type LocationHeadRead,
} from "../../settings/ProjectLocationsSettings/ProjectLocationsSettings.tsx";
import { ProjectLayrStoreSettings } from "../../settings/ProjectLayrStoreSettings/ProjectLayrStoreSettings.tsx";
import { ProjectPeers } from "../ProjectPeers/ProjectPeers.tsx";

export default observer(function ProjectFoldersDialog({
  project,
  initialServer,
  initialBrowse,
  initialHeads,
  peers,
}: {
  project: LocationProjectView;
  peers: ReturnType<typeof createProjectPeersStore>;
  initialServer?: string | undefined;
  initialBrowse?: DirectoryBrowse | undefined;
  initialHeads?: LocationHeadRead[] | undefined;
}) {
  const store = useServerStore();
  const snapshot = resourceData(store.multiServer().current);
  const servers = snapshot ? discoveryProjectServers(snapshot) : [];
  return (
    <Tabs defaultValue="folders">
      <Tabs.List>
        <Tabs.Tab value="folders">Folders</Tabs.Tab>
        <Tabs.Tab value="peers">Peers</Tabs.Tab>
      </Tabs.List>
      <Tabs.Panel value="folders" pt="md">
        <ProjectLocationsSettings
          project={project}
          servers={servers.length ? servers : undefined}
          initialServer={initialServer}
          initialBrowse={initialBrowse}
          initialHeads={initialHeads}
        />
        <ProjectLayrStoreSettings project={project} />
      </Tabs.Panel>
      <Tabs.Panel value="peers" pt="md">
        <ProjectPeers form={peers} />
      </Tabs.Panel>
    </Tabs>
  );
});

export function isLocatedProject(
  project: ProjectModel | undefined,
): project is ProjectModel & LocationProjectView {
  return !!project?.path && !!project.name;
}

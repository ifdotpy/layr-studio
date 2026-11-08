import type { ProjectModel } from "../../../app/contracts.ts";
import { Tabs } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { useState } from "react";
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
  saved,
}: {
  project: LocationProjectView;
  initialServer?: string | undefined;
  initialBrowse?: DirectoryBrowse | undefined;
  initialHeads?: LocationHeadRead[] | undefined;
  saved: () => Promise<void>;
}) {
  const store = useServerStore();
  const snapshot = resourceData(store.multiServer().current);
  const servers = snapshot ? discoveryProjectServers(snapshot) : [];
  const [error, setError] = useState("");
  return (
    <Tabs defaultValue="folders">
      <Tabs.List>
        <Tabs.Tab value="folders">Folders</Tabs.Tab>
        <Tabs.Tab value="peers">Peers</Tabs.Tab>
      </Tabs.List>
      <Tabs.Panel value="folders" pt="md">
        {error && <p role="alert">{error}</p>}
        <ProjectLocationsSettings
          project={project}
          servers={servers.length ? servers : undefined}
          initialServer={initialServer}
          initialBrowse={initialBrowse}
          initialHeads={initialHeads}
          saved={saved}
        />
        <ProjectLayrStoreSettings project={project} saved={saved} />
      </Tabs.Panel>
      <Tabs.Panel value="peers" pt="md">
        <ProjectPeers projectId={project.id} notify={setError} />
      </Tabs.Panel>
    </Tabs>
  );
});

export function isLocatedProject(
  project: ProjectModel | undefined,
): project is ProjectModel & LocationProjectView {
  return !!project?.path && !!project.name;
}

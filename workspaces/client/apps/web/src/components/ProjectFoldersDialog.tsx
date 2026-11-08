import type { ProjectModel } from "../app/contracts.ts";
import { Tabs } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { useEffect, useState } from "react";
import { useServerApi } from "../servers/ServerServicesContext.tsx";
import { errorText } from "../api.ts";
import {
  readProjectServers,
  type LocationProjectView,
  type ProjectServerChoice,
} from "../servers/projectLocations.ts";
import ProjectLocationsSettings from "./ProjectLocationsSettings.tsx";
import { ProjectLayrStoreSettings } from "./ProjectLayrStoreSettings.tsx";
import { ProjectPeers } from "./ProjectPeers.tsx";

export default observer(function ProjectFoldersDialog({
  project,
  initialServer,
  saved,
}: {
  project: LocationProjectView;
  initialServer?: string | undefined;
  saved: () => Promise<void>;
}) {
  const { get } = useServerApi();
  const [servers, setServers] = useState<ProjectServerChoice[]>([]),
    [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    void readProjectServers(get)
      .then((rows) => {
        if (active) setServers(rows);
      })
      .catch((error) => {
        if (active) setError(errorText(error));
      });
    return () => {
      active = false;
    };
  }, []);
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

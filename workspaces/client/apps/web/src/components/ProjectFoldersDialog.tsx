import { useServerApi, useServerSync } from "../servers/ServerServicesContext";
import { Tabs } from "@mantine/core";
import { useEffect, useState } from "react";
import { errorText } from "../api";
import {
  readProjectServers,
  type LocationProject,
  type ProjectServerChoice,
} from "../servers/projectLocations";
import ProjectLocationsSettings from "./ProjectLocationsSettings";
import { ProjectLayrStoreSettings } from "./ProjectLayrStoreSettings";
import { ProjectPeers } from "./ProjectPeers";

export default function ProjectFoldersDialog({
  project: initial,
  initialServer,
  saved,
}: {
  project: LocationProject;
  initialServer?: string | undefined;
  saved: () => Promise<void>;
}) {
  const { get } = useServerApi();
  const { subscribeStateProjection } = useServerSync();

  const [project, setProject] = useState(initial);
  const [servers, setServers] = useState<ProjectServerChoice[]>([]);
  const [error, setError] = useState("");
  useEffect(
    () =>
      subscribeStateProjection(
        (snapshot) => {
          const next = snapshot?.runtime.projects.find(
            (row) => row.id === initial.id,
          );
          if (next)
            setProject((current) => ({
              ...current,
              ...next,
              path: next.path ?? current.path,
              name: next.name ?? current.name,
            }));
        },
        () => {},
      ),
    [initial.id, subscribeStateProjection],
  );
  const refreshProject = async () => {
    const result = await get("/api/projects");
    const next = result.items.find((row) => row.id === project.id);
    if (next) setProject(next);
    await saved();
  };
  useEffect(() => {
    let active = true;
    void readProjectServers(get)
      .then((rows) => {
        if (active) setServers(rows);
      })
      .catch((failure) => {
        if (active) setError(errorText(failure));
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
          key={project.locations?.map((row) => row.serverId).join(":")}
          project={project}
          servers={servers.length ? servers : undefined}
          initialServer={initialServer}
          saved={refreshProject}
        />
        <ProjectLayrStoreSettings project={project} saved={refreshProject} />
      </Tabs.Panel>
      <Tabs.Panel value="peers" pt="md">
        <ProjectPeers projectId={project.id} notify={setError} />
      </Tabs.Panel>
    </Tabs>
  );
}

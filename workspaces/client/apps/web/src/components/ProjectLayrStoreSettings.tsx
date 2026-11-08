import { observer } from "mobx-react-lite";
import { useEffect, useState } from "react";
import type { GetResult } from "../api";
import {
  useServerApi,
  useServerResources,
} from "../servers/ServerServicesContext";
import type { LocationProjectView as LocationProject } from "../servers/projectLocations";
import { LayrStoreDirectory } from "./LayrStoreDirectory";

export const ProjectLayrStoreSettings = observer(
  function ProjectLayrStoreSettings({
    project,
    saved,
  }: {
    project: LocationProject;
    saved: () => Promise<void>;
  }) {
    const { post } = useServerApi();
    const resources = useServerResources();
    const [configuration, setConfiguration] =
      useState<GetResult<"/api/native-layr/settings"> | null>(null);
    useEffect(
      () =>
        resources.watchResourceChanges(
          { kind: "native-layr-settings" },
          (version) => {
            if (version?.nativeLayrSettings)
              setConfiguration(version.nativeLayrSettings);
          },
        ),
      [resources],
    );
    if (!configuration) return null;
    return (
      <LayrStoreDirectory
        label="Project store directory"
        path={project.layrStoreRoot ?? configuration.storeRoot}
        source={
          project.layrStoreRoot
            ? "project"
            : configuration.configured
              ? "default"
              : "built-in"
        }
        overridden={!!project.layrStoreRoot}
        save={async (layr_store_root) => {
          await post("/api/projects", {
            action: "set_layr_store_root",
            path: project.id,
            expected_revision: project.layrStoreRootRevision ?? 0,
            layr_store_root,
          });
          await saved();
        }}
      />
    );
  },
);

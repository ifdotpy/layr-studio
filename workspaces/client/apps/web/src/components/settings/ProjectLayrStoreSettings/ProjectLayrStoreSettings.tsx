import { resourceData } from "../../../store/runtime.ts";
import { observer } from "mobx-react-lite";
import {
  useServerApi,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { LocationProjectView as LocationProject } from "../../../servers/projectLocations.ts";
import { LayrStoreDirectory } from "../../projects/LayrStoreDirectory/LayrStoreDirectory.tsx";

export const ProjectLayrStoreSettings = observer(
  function ProjectLayrStoreSettings({
    project,
    saved,
  }: {
    project: LocationProject;
    saved: () => Promise<void>;
  }) {
    const { post } = useServerApi();
    const store = useServerStore();
    const configuration = resourceData(store.nativeLayrSettings().current);
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

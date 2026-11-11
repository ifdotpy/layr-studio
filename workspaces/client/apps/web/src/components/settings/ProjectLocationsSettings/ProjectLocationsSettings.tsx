import { flowResult } from "mobx";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { observer } from "mobx-react-lite";
import {
  useServerOwner,
  useServerNet,
  useServerApi,
  useServerStorage,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { ActionIcon, Button, Menu } from "@mantine/core";
import { MoreHorizontal } from "lucide-react";
import { useRef, useState } from "react";
import {
  type LocationProjectView as LocationProject,
  type ProjectServerChoice,
} from "../../../servers/projectLocations.ts";
import ProjectDirectoryPicker, {
  browseDirectory,
  type DirectoryBrowse,
} from "../../projects/ProjectDirectoryPicker/ProjectDirectoryPicker.tsx";
import {
  closeMenuFromTriggerOnEscape,
  restoreMenuTriggerFocusAfterClose,
  useMenuEscapeSuppression,
} from "../../ui/support/closeMenuFromTriggerOnEscape.ts";

import { SettingsRow } from "../../ui/SettingsRow/SettingsRow.tsx";
import { createProjectLocationsStore } from "./ProjectLocationsStore.ts";
export type { LocationHeadRead } from "./ProjectLocationsStore.ts";
import type { LocationHeadRead } from "./ProjectLocationsStore.ts";

type Props = {
  project: LocationProject;
  servers?: ProjectServerChoice[] | undefined;
  initialServer?: string | undefined;
  initialBrowse?: DirectoryBrowse | undefined;
  initialHeads?: LocationHeadRead[] | undefined;
};
function ProjectLocationsSettings(props: Props) {
  const owner = useServerOwner();
  return (
    <ProjectLocationsView
      key={`${owner?.id || "local"}:${props.project.id}`}
      {...props}
    />
  );
}
const ProjectLocationsView = observer(function ProjectLocationsView({
  project,
  servers,
  initialServer,
  initialBrowse,
  initialHeads,
}: Props) {
  const [form] = useState(() => createProjectLocationsStore(initialHeads));
  const [adding, setAdding] = useState(initialBrowse);
  const net = useServerNet();
  const scope = {
    projectId: project.id,
    services: {
      api: useServerApi(),
      store: useServerStore(),
      storage: useServerStorage(),
    },
  };
  const { error, busy, openLocationMenu, gitHead, change } = form;
  const locationMenuTriggers = useRef(new Map<string, HTMLButtonElement>());
  const { suppressNextEscape, clearSuppression } = useMenuEscapeSuppression(
    openLocationMenu !== null,
  );
  return (
    <div className="project-locations" aria-label="Project folders">
      {(project.locations || []).map((location) => (
        <SettingsRow
          className="project-location-row"
          key={location.serverId}
          label={
            <div>
              <strong>
                {servers?.find(
                  (row) => (row.serverId || row.id) === location.serverId,
                )?.label || location.serverId}
              </strong>
              <p>{location.path}</p>
              {location.requestedPath && (
                <small>Selected folder: {location.requestedPath}</small>
              )}
              {gitHead(project.id, location.serverId, location.path) && (
                <small>
                  HEAD {gitHead(project.id, location.serverId, location.path)}
                </small>
              )}
            </div>
          }
        >
          <Menu
            opened={openLocationMenu === location.serverId}
            onChange={(nextOpened) => {
              form.menu(scope, location.serverId, location.path, nextOpened);
              if (!nextOpened)
                restoreMenuTriggerFocusAfterClose(
                  locationMenuTriggers.current.get(location.serverId) ?? null,
                );
            }}
            withinPortal={false}
            withInitialFocusPlaceholder={false}
          >
            <Menu.Target>
              <ActionIcon
                ref={(node) => {
                  if (node)
                    locationMenuTriggers.current.set(location.serverId, node);
                  else locationMenuTriggers.current.delete(location.serverId);
                }}
                className="project-location-actions"
                size={28}
                onClick={(event) =>
                  event.currentTarget.focus({ preventScroll: true })
                }
                data-mantine-stop-propagation={
                  openLocationMenu === location.serverId || suppressNextEscape
                    ? "true"
                    : undefined
                }
                onKeyDown={(event) =>
                  closeMenuFromTriggerOnEscape(
                    event,
                    openLocationMenu === location.serverId,
                    () =>
                      form.menu(scope, location.serverId, location.path, false),
                    suppressNextEscape,
                    clearSuppression,
                  )
                }
                onBlur={clearSuppression}
                aria-label={`Options for folder ${location.path}`}
                disabled={busy}
              >
                <MoreHorizontal size={16} />
              </ActionIcon>
            </Menu.Target>
            <Menu.Dropdown data-mantine-stop-propagation="true">
              <Menu.Item
                disabled={(project.locations?.length || 0) < 2}
                onClick={() => void form.remove(scope, location.serverId)}
              >
                Remove
              </Menu.Item>
            </Menu.Dropdown>
          </Menu>
        </SettingsRow>
      ))}
      {error && <p role="alert">{error}</p>}
      {adding ? (
        <ProjectDirectoryPicker
          initialBrowse={adding}
          serverChoices={servers}
          initialServer={initialServer}
          projectId={project.id}
          submitLabel="Add folder"
          onSelect={(path, server) =>
            flowResult(change(scope, "add_location", server, path))
          }
        />
      ) : (
        <Button
          variant="light"
          disabled={busy}
          onClick={() =>
            setAdding(
              browseDirectory(
                net,
                undefined,
                initialServer || servers?.[0]?.id || "local",
                project.id,
              ),
            )
          }
        >
          Add folder
        </Button>
      )}
    </div>
  );
});
export default ProjectLocationsSettings;

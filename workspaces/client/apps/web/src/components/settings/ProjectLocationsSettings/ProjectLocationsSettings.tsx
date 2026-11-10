import { useReadView } from "../../ui/support/useReadView.ts";
import { resourceData, type ReadSlot } from "../../../store/runtime.ts";
import type { GetResult } from "../../../api.ts";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { observer } from "mobx-react-lite";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  useServerNet,
  useServerApi,
  useServerStorage,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { ActionIcon, Button, Menu } from "@mantine/core";
import { MoreHorizontal } from "lucide-react";
import { useRef, useState } from "react";
import { errorText } from "../../../api.ts";
import {
  saveProjectLocation,
  ProjectReceiptRefused,
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
export type LocationHeadRead = {
  project: string;
  server: string;
  path: string;
  slot: ReadSlot<GetResult<"/api/project-locations">>;
};

const ProjectLocationsSettings = observer(function ProjectLocationsSettings({
  project,
  servers,
  initialServer,
  initialBrowse,
  initialHeads = [],
}: {
  project: LocationProject;
  servers?: ProjectServerChoice[] | undefined;
  initialServer?: string | undefined;
  initialBrowse?: DirectoryBrowse | undefined;
  initialHeads?: LocationHeadRead[] | undefined;
}) {
  const net = useServerNet();
  const api = useServerApi(),
    store = useServerStore();
  const { post, refreshSession } = api;
  const storage = useServerStorage();

  const [adding, setAdding] = useState(initialBrowse);
  const [heads, setHeads] = useReadView(initialHeads);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [openLocationMenu, setOpenLocationMenu] = useState<string | null>(null);
  const locationMenuTriggers = useRef(new Map<string, HTMLButtonElement>());
  const { suppressNextEscape, clearSuppression } = useMenuEscapeSuppression(
    openLocationMenu !== null,
  );
  const request = useRef<{
    id: string;
    server: string;
    path?: string | undefined;
    action: "add_location" | "remove_location";
  } | null>(null);
  const readHead = (server: string, path: string) => {
    const slot = store.reads.projectLocations(api, {
      project: project.id,
      server,
      path,
      action: "info",
    });
    setHeads([
      ...(heads || []).filter((row) => row.server !== server),
      { project: project.id, server, path, slot },
    ]);
    return slot.read();
  };
  const gitHead = (server: string, path: string) =>
    resourceData(
      (heads || []).find(
        (row) =>
          row.project === project.id &&
          row.server === server &&
          row.path === path,
      )?.slot.current || { status: "absent" },
    )?.gitHead;
  const change = async (
    action: "add_location" | "remove_location",
    server: string,
    path?: string,
  ) => {
    const old = request.current;
    if (
      old &&
      (old.server !== server || old.path !== path || old.action !== action)
    )
      throw new Error(
        "Retry the pending location request before another change.",
      );
    request.current ||= { action, server, path, id: randomUUID() };
    await saveProjectLocation(
      {
        action,
        server,
        path,
        project: project.id,
        request_id: request.current.id,
      },
      { post, refreshSession, storage },
    ).catch((error: unknown) => {
      if (error instanceof ProjectReceiptRefused) request.current = null;
      throw error;
    });
    request.current = null;
    if (action === "add_location" && path) {
      // Read only the folder added by this explicit user action.
      await readHead(server, path).catch(() => {});
    }
  };
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
              {gitHead(location.serverId, location.path) && (
                <small>HEAD {gitHead(location.serverId, location.path)}</small>
              )}
            </div>
          }
        >
          <Menu
            opened={openLocationMenu === location.serverId}
            onChange={(nextOpened) => {
              if (nextOpened)
                void readHead(location.serverId, location.path).catch(() => {});
              setOpenLocationMenu((current) =>
                nextOpened
                  ? location.serverId
                  : current === location.serverId
                    ? null
                    : current,
              );
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
                      setOpenLocationMenu((current) =>
                        current === location.serverId ? null : current,
                      ),
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
                onClick={() => {
                  setBusy(true);
                  setError("");
                  void change("remove_location", location.serverId)
                    .catch((failure) => setError(errorText(failure)))
                    .finally(() => setBusy(false));
                }}
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
          onSelect={(path, server) => change("add_location", server, path)}
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

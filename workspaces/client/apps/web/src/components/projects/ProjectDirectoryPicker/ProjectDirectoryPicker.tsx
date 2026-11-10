import { useReadView } from "../../ui/support/useReadView.ts";
import { observer } from "mobx-react-lite";
import { observable, runInAction } from "mobx";
import { resourceData, type ReadSlot } from "../../../store/runtime.ts";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { GetResult } from "../../../api.ts";
import { useServerNet } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useServerFileDialogs } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";

import {
  ActionIcon,
  Button,
  Select,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import { ArrowUp, ChevronRight, Folder, Search } from "lucide-react";
import { useState } from "react";
import { errorText } from "../../../api.ts";
import type { paths } from "../../../generated/api.ts";
import {
  discoveryProjectServers,
  type ProjectServerChoice,
} from "../../../servers/projectLocations.ts";

/** The parent of an absolute path, by its text; the root is its own parent. */
export const parentPath = (path: string) =>
  path.replace(/\/+$/, "").replace(/\/[^/]*$/, "") || "/";

type Directory =
  paths["/api/directories"]["get"]["responses"][200]["content"]["application/json"];
export type DirectoryBrowse = {
  path?: string | undefined;
  directory: ReadSlot<Directory>;
  matches: ReadSlot<GetResult<"/api/project-locations">> | null;
};
export function browseDirectory(
  net: ReturnType<typeof useServerNet>,
  path?: string,
  server = "local",
  project?: string,
): DirectoryBrowse {
  const query = { path, ...(server !== "local" ? { server } : {}) };
  const browse: DirectoryBrowse = observable.object(
    {
      path,
      directory: net.reads.directories(query),
      matches: null,
    },
    {},
    { deep: false },
  );
  void browse.directory
    .read()
    .then((result) => {
      if (!project) return;
      const matches = net.reads.projectLocations({
        project,
        server,
        action: "matches",
        path: result.path,
      });
      runInAction(() => {
        browse.matches = matches;
      });
      void matches.read().catch(() => {});
    })
    .catch(() => {});
  return browse;
}

export default observer(function ProjectDirectoryPicker({
  initialPath,
  initialBrowse,
  onSelect,
  serverChoices,
  initialServer,
  projectId,
  showServerSelector = true,
  submitLabel = "Add project",
}: {
  initialPath?: string | undefined;
  initialBrowse?: DirectoryBrowse | undefined;
  onSelect: (path: string, server: string) => Promise<void>;
  serverChoices?: ProjectServerChoice[] | undefined;
  initialServer?: string | undefined;
  projectId?: string;
  showServerSelector?: boolean;
  submitLabel?: string;
}) {
  const { canPickDirectory, pickDirectory } = useServerFileDialogs();
  const net = useServerNet(),
    store = useServerStore();

  const discovery = serverChoices ? undefined : store.multiServer().current;
  const snapshot = discovery && resourceData(discovery);
  const servers =
    serverChoices ??
    (snapshot
      ? discoveryProjectServers(snapshot)
      : [{ id: "local", label: "This computer" }]);
  const [server, setServer] = useState(
    initialServer || serverChoices?.[0]?.id || "local",
  );
  const serverError =
    discovery?.status === "error" ? errorText(discovery.error) : "";
  const [browse, setBrowse] = useReadView(initialBrowse || null);
  const matchRead = browse?.matches;
  const matches = (
    matchRead ? resourceData(matchRead.current)?.matches || [] : []
  ).map((row) => row.path);
  const selectedServer = servers.find((row) => row.id === server);
  const path = browse?.path || initialPath;
  const [typedPath, setTypedPath] = useState<string>();
  const directoryRead = browse?.directory;
  const directory = directoryRead
    ? resourceData(directoryRead.current)
    : undefined;
  const [query, setQuery] = useState("");
  const loading = !directoryRead || directoryRead.current.status === "loading";
  const [saving, setSaving] = useState(false);
  const error =
    directoryRead?.current.status === "error"
      ? errorText(directoryRead.current.error)
      : "";
  const [selectionError, setSelectionError] = useState("");
  const navigate = (next?: string, nextServer = server) => {
    setSelectionError("");
    setQuery("");
    setTypedPath(undefined);
    setBrowse(browseDirectory(net, next, nextServer, projectId));
  };
  const rows = directory?.directories.filter((row) =>
    row.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()),
  );
  const enteredPath = typedPath ?? directory?.path ?? path ?? "";
  const pathChanged = !!directory && enteredPath.trim() !== directory.path;
  return (
    <div
      className="directory-picker"
      aria-busy={loading || saving}
      data-server-selector={showServerSelector}
    >
      {showServerSelector && (
        <Select
          label="Server"
          value={server}
          disabled={saving}
          data={servers.map((row) => ({
            value: row.id,
            label: row.label + (row.disabled ? " (Offline)" : ""),
            disabled: !!row.disabled,
          }))}
          onChange={(next) => {
            if (!next || next === server) return;
            setServer(next);
            void navigate(undefined, next);
          }}
        />
      )}
      {serverError && <p role="alert">{serverError}</p>}
      <form
        className="directory-path-entry"
        onSubmit={(event) => {
          event.preventDefault();
          void navigate(enteredPath.trim() || undefined);
        }}
      >
        <TextInput
          label="Folder path"
          placeholder="/Users/…/Projects/…"
          value={enteredPath}
          onChange={(event) => setTypedPath(event.currentTarget.value)}
          disabled={saving}
        />
        <Button type="submit" disabled={saving}>
          Open path
        </Button>
      </form>
      {canPickDirectory() && server === "local" && (
        <Button
          variant="light"
          onClick={() => {
            void pickDirectory()
              .then((folder) => {
                if (folder) navigate(folder);
              })
              .catch((failure) => setSelectionError(errorText(failure)));
          }}
          disabled={saving}
        >
          Browse folders…
        </Button>
      )}
      <div className="directory-location">
        <ActionIcon
          aria-label="Parent folder"
          disabled={loading || saving || !directory?.parent}
          onClick={() => directory?.parent && navigate(directory.parent)}
        >
          <ArrowUp size={18} />
        </ActionIcon>
        <span>{loading || error ? path || "Projects" : directory?.path}</span>
      </div>
      <TextInput
        aria-label="Filter folders"
        placeholder="Filter folders"
        leftSection={<Search size={16} />}
        value={query}
        disabled={loading || saving}
        onChange={(event) => setQuery(event.currentTarget.value)}
      />
      <div className="directory-list" aria-label="Folders">
        {loading ? (
          <p role="status">Loading folders…</p>
        ) : error ? (
          <div role="alert">
            <p>{error}</p>
            <Button onClick={() => void navigate(path)}>Retry</Button>
            {directory && directory.path !== path && (
              <Button onClick={() => navigate(directory.path)}>
                Back to previous folder
              </Button>
            )}
            {/* A saved path that does not exist yet opens from its parent. */}
            {path && parentPath(path) !== path && (
              <Button onClick={() => navigate(parentPath(path))}>
                Open parent folder
              </Button>
            )}
          </div>
        ) : rows?.length ? (
          rows.map((row) => (
            <UnstyledButton
              className="directory-row"
              key={row.path}
              disabled={saving}
              onClick={() => navigate(row.path)}
            >
              <Folder size={17} />
              <span>
                {row.name}
                {matches.includes(row.path) && (
                  <small> (same git origin)</small>
                )}
              </span>
              <ChevronRight size={15} />
            </UnstyledButton>
          ))
        ) : (
          <p>{query ? "No matching folders" : "No folders inside"}</p>
        )}
      </div>
      {selectionError && (
        <p className="directory-error" role="alert">
          {selectionError}
        </p>
      )}
      <div className="directory-footer">
        {!loading && !error && pathChanged && (
          <p role="status">Select Open path to open the entered folder.</p>
        )}
        <Button
          variant="filled"
          loading={saving}
          disabled={
            loading ||
            !!error ||
            !directory ||
            pathChanged ||
            !!selectedServer?.disabled
          }
          onClick={async () => {
            if (!directory || saving) return;
            setSaving(true);
            setSelectionError("");
            try {
              await onSelect(directory.path, server);
            } catch (failure) {
              setSelectionError(errorText(failure));
            } finally {
              setSaving(false);
            }
          }}
        >
          {submitLabel}
        </Button>
      </div>
    </div>
  );
});

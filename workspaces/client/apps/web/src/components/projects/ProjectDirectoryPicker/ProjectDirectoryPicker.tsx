import { AsyncFeedback } from "../../../stores/AsyncFeedback.ts";
import { whileObserved } from "../../../observed.ts";
import { textField } from "../../../stores/fields.ts";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { observer } from "mobx-react-lite";
import { flow, observable, makeAutoObservable, observableRef } from "mobx";
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
  void flow(function* (): Generator<Promise<unknown>, void, Directory> {
    try {
      const result = yield browse.directory.read();
      if (!project) return;
      const matches = net.reads.projectLocations({
        project,
        server,
        action: "matches",
        path: result.path,
      });
      browse.matches = matches;
      yield matches.read();
    } catch {
      /* Read slots own the displayed failure. */
    }
  })();
  return browse;
}

type Props = {
  initialPath?: string | undefined;
  initialBrowse?: DirectoryBrowse | undefined;
  onSelect: (path: string, server: string) => Promise<void>;
  serverChoices?: ProjectServerChoice[] | undefined;
  initialServer?: string | undefined;
  projectId?: string;
  showServerSelector?: boolean;
  submitLabel?: string;
};
function createDirectoryPicker(
  props: Props,
  net: ReturnType<typeof useServerNet>,
  store: ReturnType<typeof useServerStore>,
  dialogs: ReturnType<typeof useServerFileDialogs>,
) {
  const initial: {
    browse: DirectoryBrowse | null;
    typedPath: string | undefined;
  } = { browse: props.initialBrowse || null, typedPath: undefined };
  let active = false;
  const draft = makeAutoObservable(
    {
      server: props.initialServer || props.serverChoices?.[0]?.id || "local",
      ...initial,
      query: "",
      feedback: new AsyncFeedback(),
      get directory(): Directory | undefined {
        return this.browse
          ? resourceData(this.browse.directory.current)
          : undefined;
      },
      view({ serverChoices, initialPath }: Props) {
        const { browse, server, typedPath, query } = this;
        const discovery = serverChoices
          ? undefined
          : store.multiServer().current;
        const snapshot = discovery && resourceData(discovery);
        const servers =
          serverChoices ??
          (snapshot
            ? discoveryProjectServers(snapshot)
            : [{ id: "local", label: "This computer" }]);
        const serverError =
          discovery?.status === "error" ? errorText(discovery.error) : "";
        const matchRead = browse?.matches;
        const matches = (
          matchRead ? resourceData(matchRead.current)?.matches || [] : []
        ).map((row) => row.path);
        const selectedServer = servers.find((row) => row.id === server);
        const path = browse?.path || initialPath;
        const directoryRead = browse?.directory;
        const directory = this.directory;
        const loading =
          !directoryRead || directoryRead.current.status === "loading";
        const error =
          directoryRead?.current.status === "error"
            ? errorText(directoryRead.current.error)
            : "";
        const rows = directory?.directories.filter((row) =>
          row.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()),
        );
        const enteredPath = typedPath ?? directory?.path ?? path ?? "";
        const pathChanged =
          !!directory && enteredPath.trim() !== directory.path;

        return {
          servers,
          serverError,
          directory,
          loading,
          error,
          rows,
          enteredPath,
          pathChanged,
          path,
          matches,
          selectedServer,
        };
      },

      setTypedPath(value: string) {
        this.typedPath = value;
      },
      navigate(
        next: string | undefined,
        projectId?: string,
        nextServer?: string,
      ) {
        this.feedback.error = "";
        this.query = "";
        this.typedPath = undefined;
        this.browse = browseDirectory(
          net,
          next,
          nextServer ?? this.server,
          projectId,
        );
      },
      selectServer(next: string | null, projectId?: string) {
        if (!next || next === this.server) return;
        this.server = next;
        this.navigate(undefined, projectId, next);
      },
      pick(projectId?: string) {
        return this.feedback.run(
          () => dialogs.pickDirectory(),
          (path) => {
            if (path && active) this.navigate(path, projectId);
          },
        );
      },
      select(props: Props) {
        const directory = this.directory,
          server = this.server,
          selected = this.view(props).selectedServer;
        return directory && selected && !selected.disabled
          ? this.feedback.run(() => props.onSelect(directory.path, server))
          : undefined;
      },
    },
    { browse: observableRef, view: false },
    { autoBind: true },
  );
  whileObserved(draft, "directory", () => {
    active = true;
    return () => {
      active = false;
    };
  });
  return draft;
}
export default observer(function ProjectDirectoryPicker(props: Props) {
  const net = useServerNet(),
    store = useServerStore(),
    dialogs = useServerFileDialogs();
  const { showServerSelector = true, submitLabel = "Add project" } = props;
  return (
    <Owned
      key={props.projectId}
      create={() => createDirectoryPicker(props, net, store, dialogs)}
    >
      {(draft) => {
        const view = draft.view(props);
        const { directory, path, pathChanged } = view;
        const navigate = (next?: string) =>
          draft.navigate(next, props.projectId);
        const { canPickDirectory } = dialogs;
        return (
          <div
            className="directory-picker"
            aria-busy={view.loading || draft.feedback.pending}
            data-server-selector={showServerSelector}
          >
            {showServerSelector && (
              <Select
                label="Server"
                value={draft.server}
                disabled={draft.feedback.pending}
                data={view.servers.map((row) => ({
                  value: row.id,
                  label: row.label + (row.disabled ? " (Offline)" : ""),
                  disabled: !!row.disabled,
                }))}
                onChange={(next) => draft.selectServer(next, props.projectId)}
              />
            )}
            {view.serverError && <p role="alert">{view.serverError}</p>}
            <form
              className="directory-path-entry"
              onSubmit={(event) => {
                event.preventDefault();
                void navigate(view.enteredPath.trim() || undefined);
              }}
            >
              <TextInput
                label="Folder path"
                placeholder="/Users/…/Projects/…"
                value={view.enteredPath}
                onChange={(event) =>
                  draft.setTypedPath(event.currentTarget.value)
                }
                disabled={draft.feedback.pending}
              />
              <Button type="submit" disabled={draft.feedback.pending}>
                Open path
              </Button>
            </form>
            {canPickDirectory() && draft.server === "local" && (
              <Button
                variant="light"
                onClick={() => void draft.pick(props.projectId)}
                disabled={draft.feedback.pending}
              >
                Browse folders…
              </Button>
            )}
            <div className="directory-location">
              <ActionIcon
                aria-label="Parent folder"
                disabled={
                  view.loading || draft.feedback.pending || !directory?.parent
                }
                onClick={() => directory?.parent && navigate(directory.parent)}
              >
                <ArrowUp size={18} />
              </ActionIcon>
              <span>
                {view.loading || view.error
                  ? path || "Projects"
                  : directory?.path}
              </span>
            </div>
            <TextInput
              aria-label="Filter folders"
              placeholder="Filter folders"
              leftSection={<Search size={16} />}
              {...textField(draft, "query")}
              disabled={view.loading || draft.feedback.pending}
            />
            <div className="directory-list" aria-label="Folders">
              {view.loading ? (
                <p role="status">Loading folders…</p>
              ) : view.error ? (
                <div role="alert">
                  <p>{view.error}</p>
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
              ) : view.rows?.length ? (
                view.rows.map((row) => (
                  <UnstyledButton
                    className="directory-row"
                    key={row.path}
                    disabled={draft.feedback.pending}
                    onClick={() => navigate(row.path)}
                  >
                    <Folder size={17} />
                    <span>
                      {row.name}
                      {view.matches.includes(row.path) && (
                        <small> (same git origin)</small>
                      )}
                    </span>
                    <ChevronRight size={15} />
                  </UnstyledButton>
                ))
              ) : (
                <p>
                  {draft.query ? "No matching folders" : "No folders inside"}
                </p>
              )}
            </div>
            {draft.feedback.error && (
              <p className="directory-error" role="alert">
                {draft.feedback.error}
              </p>
            )}
            <div className="directory-footer">
              {!view.loading && !view.error && pathChanged && (
                <p role="status">
                  Select Open path to open the entered folder.
                </p>
              )}
              <Button
                variant="filled"
                loading={draft.feedback.pending}
                disabled={
                  view.loading ||
                  !!view.error ||
                  !directory ||
                  pathChanged ||
                  !view.selectedServer ||
                  !!view.selectedServer.disabled
                }
                onClick={() => void draft.select(props)}
              >
                {submitLabel}
              </Button>
            </div>
          </div>
        );
      }}
    </Owned>
  );
});

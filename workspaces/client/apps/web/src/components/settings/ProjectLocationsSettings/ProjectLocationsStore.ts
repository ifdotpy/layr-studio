import { makeAutoObservable, observableRef } from "mobx";
import { resourceData, type ReadSlot } from "../../../store/runtime.ts";
import { errorText, type GetResult } from "../../../api.ts";
import type { ServerServices } from "../../../servers/serverServices.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  saveProjectLocation,
  ProjectReceiptRefused,
} from "../../../servers/projectLocations.ts";

export type LocationHeadRead = {
  project: string;
  server: string;
  path: string;
  slot: ReadSlot<GetResult<"/api/project-locations">>;
};

export type LocationScope = {
  projectId: string;
  services: Pick<ServerServices, "api" | "store" | "storage">;
};

export function createProjectLocationsStore(heads: LocationHeadRead[] = []) {
  let request: {
    id: string;
    server: string;
    path?: string | undefined;
    action: "add_location" | "remove_location";
  } | null = null;
  const form = {
    heads,
    error: "",
    busy: false,
    openLocationMenu: null as string | null,
    menu(scope: LocationScope, server: string, path: string, opened: boolean) {
      if (opened) void this.readHead(scope, server, path).catch(() => {});
      if (opened || this.openLocationMenu === server)
        this.openLocationMenu = opened ? server : null;
    },
    *remove(scope: LocationScope, server: string) {
      this.busy = true;
      this.error = "";
      try {
        yield this.change(scope, "remove_location", server);
      } catch (failure) {
        this.error = errorText(failure);
      } finally {
        this.busy = false;
      }
    },
    readHead(
      { projectId, services }: LocationScope,
      server: string,
      path: string,
    ) {
      const slot = services.store.reads.projectLocations(services.api, {
        project: projectId,
        server,
        path,
        action: "info",
      });
      this.heads = [
        ...this.heads.filter((row) => row.server !== server),
        { project: projectId, server, path, slot },
      ];
      return slot.read();
    },
    gitHead(projectId: string, server: string, path: string) {
      return resourceData(
        this.heads.find(
          (row) =>
            row.project === projectId &&
            row.server === server &&
            row.path === path,
        )?.slot.current || { status: "absent" },
      )?.gitHead;
    },
    *change(
      scope: LocationScope,
      action: "add_location" | "remove_location",
      server: string,
      path?: string,
    ) {
      const { projectId, services } = scope;
      const old = request;
      if (
        old &&
        (old.server !== server || old.path !== path || old.action !== action)
      )
        throw new Error(
          "Retry the pending location request before another change.",
        );
      request ||= { action, server, path, id: randomUUID() };
      try {
        yield saveProjectLocation(
          {
            action,
            server,
            path,
            project: projectId,
            request_id: request.id,
          },
          {
            post: services.api.post,
            refreshSession: services.api.refreshSession,
            storage: services.storage,
          },
        );
      } catch (error) {
        if (error instanceof ProjectReceiptRefused) request = null;
        throw error;
      }
      request = null;
      if (action === "add_location" && path) {
        // Read only the folder added by this explicit user action.
        yield this.readHead(scope, server, path).catch(() => {});
      }
    },
  };
  return makeAutoObservable(form, { heads: observableRef }, { autoBind: true });
}

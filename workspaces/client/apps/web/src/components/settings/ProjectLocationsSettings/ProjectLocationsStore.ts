import { MenuEscapeState } from "../../ui/support/closeMenuFromTriggerOnEscape.ts";
import { makeAutoObservable, observableRef } from "mobx";
import { resourceData, type ReadSlot } from "../../../store/runtime.ts";
import { errorText, type GetResult } from "../../../api.ts";
import type { ServerServices } from "../../../servers/serverServices.ts";
import { projectLocationRequest } from "../../../servers/projectLocations.ts";
import type { RequestEntry } from "../../../servers/RequestRegistry.ts";

export type LocationHeadRead = {
  project: string;
  server: string;
  path: string;
  slot: ReadSlot<GetResult<"/api/project-locations">>;
};

export type LocationScope = {
  projectId: string;
  services: Pick<ServerServices, "api" | "store" | "storage"> & {
    requests?: ServerServices["requests"] | undefined;
  };
};

export function createProjectLocationsStore(heads: LocationHeadRead[] = []) {
  const form = {
    heads,
    request: null as RequestEntry | null,
    validationError: "",
    get error() {
      return this.validationError || this.request?.view.errorText || "";
    },
    get busy() {
      return this.request?.view.busy || false;
    },
    menuState: new MenuEscapeState(),
    get openLocationMenu() {
      return this.menuState.openedKey;
    },
    menu(scope: LocationScope, server: string, path: string, opened: boolean) {
      if (opened) void this.readHead(scope, server, path).catch(() => {});
      this.menuState.changeMenu(server, opened);
    },
    *remove(scope: LocationScope, server: string) {
      this.validationError = "";
      try {
        yield this.change(scope, "remove_location", server);
      } catch (failure) {
        this.validationError = errorText(failure);
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
      this.validationError = "";
      this.request = projectLocationRequest(scope.services, {
        action,
        server,
        path,
        project: scope.projectId,
      });
      yield this.request.view.canRetry
        ? scope.services.requests!.retry(this.request.requestId)
        : this.request.completion;
      if (this.request.view.errorText)
        throw new Error(this.request.view.errorText);
    },
  };
  return makeAutoObservable(
    form,
    { heads: observableRef, request: observableRef },
    { autoBind: true },
  );
}

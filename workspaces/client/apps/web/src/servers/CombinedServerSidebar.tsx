import { makeAutoObservable, observableShallow, flowResult } from "mobx";
import { errorText } from "../api.ts";
import {
  createMergedSidebarServices,
  sidebarOwner,
} from "./sidebarServices.ts";
import { createSidebarVisualSession } from "./sidebarVisualSession.ts";
import type { CombinedSidebarServices } from "./combinedSidebarServices.ts";
import type { SidebarTarget } from "../components/sidebar/support/services.tsx";
import type { ServerCommand } from "./navigation.ts";
import { SidebarListOwner } from "../app/shell/SidebarOwner.ts";
import type {
  SidebarAreas,
  CombinedSidebarBoundary,
} from "../app/shell/SidebarOwner.ts";
import type { ShellSidebarSource } from "../app/startup/ShellSidebarSource.ts";

/** Commands resolve the owning connection at the event boundary, before transport. */
export type CombinedSidebarShell = Pick<
  ShellSidebarSource,
  | "model"
  | "current"
  | "mobileOpen"
  | "desktopCollapsed"
  | "isCompact"
  | "recordCompact"
  | "close"
  | "selectedId"
  | "navigationIntent"
  | "value"
>;
export class CombinedSidebarOwner {
  readonly source;
  readonly shell;
  readonly list: SidebarListOwner;
  readonly visualSession = createSidebarVisualSession();
  marking = new Set<string>();
  constructor(
    shell: CombinedSidebarShell,
    source: CombinedSidebarServices,
    inputs: {
      areas: SidebarAreas;
      readonly creation: CombinedSidebarBoundary["creation"];
    },
  ) {
    this.shell = shell;
    this.source = source;
    const projectedServices = () => this.projectedServices;
    this.list = new SidebarListOwner(inputs.areas, {
      get model() {
        return shell.model;
      },
      get services() {
        return projectedServices();
      },
      commands: {
        rename: (id, name) => flowResult(this.rename(id, name)),
        notify: source.notify,
      },
      run: (command) => {
        void this.run(command);
      },
      get creation() {
        return inputs.creation;
      },
      visibility: shell,
      selection: shell,
      preferences: shell,
      read: this,
    });
    makeAutoObservable(
      this,
      {
        source: false,
        shell: false,
        list: false,
        visualSession: false,
        marking: observableShallow,
      },
      { autoBind: true },
    );
  }
  get projectedServices() {
    const result = createMergedSidebarServices(
      this.shell.model,
      this.source.backends,
      this.source.online,
      this.source.aliases,
    );
    result.cache = this.visualSession(
      this.shell.model,
      result.cache,
      (failure) => this.source.notify(errorText(failure)),
    );
    return result;
  }
  action(target: SidebarTarget, command: (owner: string) => ServerCommand) {
    try {
      const owner = sidebarOwner(this.shell.model, target);
      if (!this.source.online(owner))
        throw new Error("The sidebar server is offline or unavailable.");
      this.source.send(owner, command(owner));
    } catch (failure) {
      this.source.notify(errorText(failure));
    }
  }
  *run(command: ServerCommand) {
    const { online, send, executeCommand, settings, search, notify } =
      this.source;
    const model = this.shell.model;
    if (command.action === "settings") settings();
    else if (command.action === "search") {
      this.shell.close();
      search();
    } else if (command.action === "mark-unread") {
      const reference = model.chatReferences.get(command.id);
      if (!reference || !online(reference.owner)) return;
      const id = command.id;
      this.marking.add(id);
      try {
        yield executeCommand(reference.owner, {
          ...command,
          id: reference.id,
          target: { ...command.target },
        });
      } catch (failure) {
        notify(errorText(failure));
      } finally {
        this.marking.delete(id);
      }
    } else if (command.action === "open") {
      try {
        const reference = model.requireReference(
          model.chatReferences,
          command.id,
        );
        send(reference.owner, { ...command, id: reference.id });
      } catch (failure) {
        notify(errorText(failure));
      }
    } else if (
      command.action === "change-project" ||
      command.action === "remove-chat"
    ) {
      this.action({ kind: "chat", id: command.id }, () => ({
        ...command,
        id: model.requireReference(model.chatReferences, command.id).id,
      }));
    } else if (
      command.action === "project-folders" ||
      command.action === "delete-project"
    ) {
      const path = command.projectId;
      this.action({ kind: "project", path }, (owner) => ({
        ...command,
        projectId: model.wireProject(owner, path),
      }));
    } else if (
      command.action === "new-chat" ||
      command.action === "new-shared-chat" ||
      command.action === "project-account"
    ) {
      if (command.action === "new-shared-chat") this.shell.close();
      const path = command.path;
      if (!path) {
        if (online(this.shell.current)) send(this.shell.current, command);
        else notify("The sidebar server is offline.");
        return;
      }
      const folder = command.action === "new-chat" ? command.folder : undefined;
      this.action({ kind: "project", path, folder }, (owner) => {
        if (command.action !== "new-chat")
          return { ...command, path: model.wireProject(owner, path) };
        const group = model.groups.get(path);
        const member = group?.members.find((item) => item.source.id === owner);
        const nativeFolder = folder
          ? member?.project.folders?.find(
              (item) =>
                model.folderKey(owner, member.project.path || "", item.id) ===
                folder,
            )?.id
          : undefined;
        if (folder && !nativeFolder)
          throw new Error("The folder is unavailable on this server.");
        return {
          action: "new-chat",
          path: model.wireProject(owner, path),
          ...(nativeFolder
            ? {
                folder: nativeFolder,
                projectId: group?.id,
                projectServerId:
                  group?.home.project.homeServerId || group?.owner,
              }
            : {}),
        };
      });
    } else if (command.action === "add-project") {
      if (online(this.shell.current)) send(this.shell.current, command);
      else notify("The sidebar server is offline.");
    }
  }
  *rename(id: string, name: string) {
    const backend = this.projectedServices.owner({ kind: "chat", id });
    const reference = this.shell.model.requireReference(
      this.shell.model.chatReferences,
      id,
    );
    if (!backend.rename)
      throw new Error("Rename is unavailable on this server.");
    yield backend.rename(reference.id, name);
  }

  start() {
    return this.list.start();
  }
}

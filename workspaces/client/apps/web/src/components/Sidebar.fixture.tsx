import { defaultSyncClient } from "../sync/client.ts";
import type { ComponentSnapshot as Snapshot } from "./componentTestFixtures.ts";
import { setStartupFixture } from "./observableFixture.tsx";
import "@mantine/core/styles.css";
import type { DeepReadonly } from "../types.ts";
import {
  createStateMirror,
  createProjectionMetadataMirror,
  type StateCollections,
} from "../generated/observableResources.ts";
import { intercept, observable, runInAction, spy } from "mobx";
import { useEffect, useLayoutEffect, useState } from "react";
import { useServerStore } from "../servers/ServerServicesContext.tsx";
import { DEFAULT_HIDE_OLD_CHATS_THRESHOLD } from "../studioPreferences.ts";
import { ServerAreas } from "../app/ServerAreasProvider.tsx";
import { createServerServices } from "../servers/serverServices.ts";
import { resourceData } from "../store/runtime.ts";
import { createServerSubscriptions } from "../sync/subscriptions/server.ts";
import { componentSnapshot as snapshotFixture } from "./componentTestFixtures.ts";
import Sidebar from "./Sidebar.tsx";
import {
  SidebarListOwner,
  type SidebarAreas,
} from "../app/shell/SidebarOwner.ts";
import {
  localSidebarServices,
  type SidebarServices,
} from "./sidebar/services.tsx";
import type { SidebarAgent, SidebarChatView } from "./sidebar/viewModel.ts";
import {
  type SidebarCommands,
  useSidebarCommands,
} from "./sidebar/commands.ts";
/** Scenario inputs build generated models once; events change the fixture selection. */
export type SidebarProps = {
  services?: SidebarServices;
  offlineServers?: ReadonlySet<string>;
  data?: DeepReadonly<Snapshot>;
  rows?: readonly SidebarChatView[];
  collections?: StateCollections;
  opened: string | null;
  creating: boolean;
  mobile: boolean;
  close(): void;
  markingRead: Set<string>;
  pendingReadTargets?: ReadonlyMap<
    string,
    NonNullable<SidebarAgent["markRead"]>
  >;
  hideOldChatsThreshold?: number;
};

export default function SidebarFixture(input: SidebarProps) {
  const commands = useSidebarCommands(),
    store = useServerStore();
  const [fixture] = useState(() => {
    const mirror = createStateMirror();
    mirror.apply([
      ...(input.rows || []).map((row) => ({
        collection: "agentSummary" as const,
        id: row.agent.id,
        value: row.agent,
      })),
      ...(input.data?.runtime.rooms || []).map((value) => ({
        collection: "room" as const,
        id: value.id,
        value,
      })),
      ...(input.data?.runtime.peerTeams || []).map((value) => ({
        collection: "peerTeam" as const,
        id: value.id,
        value,
      })),
      ...(input.data?.runtime.projects || []).map((project) => ({
        collection: "project" as const,
        id: project.id,
        value: project,
      })),
      {
        collection: "workspace",
        id: "fixture",
        value: { ...input.data?.runtime, stateDir: input.data?.stateDir || "" },
      },
    ]);
    const controlled = observable.box(input, { deep: false });
    const collections = observable.box(input.collections ?? mirror.current, {
      deep: false,
    });
    const services = input.services || localSidebarServices;
    const areas = sidebarFixtureAreas(
      {
        ...store,
        state: () => ({
          get current() {
            return { status: "live" as const, data: collections.get() };
          },
        }),
      },
      {
        ...services,
        available: (target) =>
          !input.offlineServers?.has(services.owner(target).ownerId) &&
          (services.available?.(target) ?? true),
      },
    );
    runInAction(() => {
      areas.navigation.selectedId = input.opened;
      areas.sidebar.mobileOpen = input.mobile;
      areas.creation.isCreating = input.creating;
      areas.preferences.value = {
        ...areas.preferences.value,
        hideOldChatsThreshold:
          input.hideOldChatsThreshold ?? DEFAULT_HIDE_OLD_CHATS_THRESHOLD,
      };
      for (const id of input.markingRead)
        areas.read.scope.requests.set(id, Promise.resolve());
      areas.read.scope.pendingReadTargets.replace([
        ...(input.pendingReadTargets ?? []),
      ]);
    });
    const stopVisibility = intercept(areas.sidebar, "mobileOpen", (change) =>
      change.newValue === controlled.get().mobile ? change : null,
    );
    const run = sidebarFixtureRun(areas, commands);
    return {
      areas,
      controlled,
      stopVisibility,
      collections,
      owner: new SidebarListOwner(areas, undefined, run),
    };
  });
  useLayoutEffect(
    () =>
      runInAction(() => {
        fixture.controlled.set(input);
        if (input.collections) fixture.collections.set(input.collections);
        fixture.areas.navigation.selectedId = input.opened;
        fixture.areas.sidebar.mobileOpen = input.mobile;
      }),
    [fixture, input.collections, input.opened, input.mobile],
  );
  useEffect(() => {
    const stopClose = spy((event) => {
      if (
        event.type === "action" &&
        event.object === fixture.areas.sidebar &&
        event.name === "close"
      )
        fixture.controlled.get().close();
    });
    const stop = fixture.owner.start();
    return () => {
      stop();
      stopClose();
      fixture.stopVisibility();
      fixture.areas.dispose();
    };
  }, [fixture]);
  return <Sidebar owner={fixture.owner} />;
}

let serial = 0;
/** Complete production owners backed by the scenario's generated resource store. */
export function sidebarFixtureAreas(
  store: SidebarAreas["scope"]["store"],
  sidebarServices: SidebarServices = localSidebarServices,
): ServerAreas {
  const services = createServerServices({
    id: `sidebar-fixture-${++serial}`,
    origin: "https://sidebar.test",
    credentialId: "fixture",
    label: "Sidebar",
  });
  Object.defineProperty(services, "sync", {
    value: {
      ...defaultSyncClient,
      projectionMetadata: createProjectionMetadataMirror({
        workspaceId: "fixture",
        ready: false,
        complete: false,
        projectSequences: {},
      }).current,
    },
  });
  Object.defineProperty(services, "sidebarServices", {
    value: sidebarServices,
  });
  const resources = createServerSubscriptions({
    api: services.api,
    tokenRates: services.tokenRates,
    sync: () => services.sync,
    owner: services.owner,
  });
  Object.defineProperty(services, "resources", { value: resources });
  const areas = new ServerAreas({ services, store, screen: services.screen });
  areas.layout.recordResize({
    host: { x: 0, y: 0, width: 1800, height: 800 },
    viewport: { x: 0, y: 0, width: 1800, height: 800 },
    rootFontPx: 16,
    shortTouchLandscape: false,
  });
  runInAction(() => {
    setStartupFixture(areas, {
      ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
      stateDir:
        resourceData(store.state().current)?.workspaces.values().next().value
          ?.stateDir ?? "",
    });
  });
  const dispose = areas.dispose;
  areas.dispose = () => {
    dispose();
    resources.dispose();
    services.close();
  };
  return areas;
}

/** Scenario commands intercept UI events; all state remains in the real owners. */
export function sidebarFixtureRun(areas: ServerAreas, c: SidebarCommands) {
  return async (command: Parameters<SidebarAreas["run"]>[0]) => {
    if (command.action === "open") {
      runInAction(() => {
        areas.navigation.selectedId = command.id;
      });
      c.open(command.id);
    } else if (command.action === "new-chat")
      c.newChat(command.path, command.folder);
    else if (command.action === "new-shared-chat") {
      areas.sidebar.close();
      c.newSharedChat?.(command.path);
    } else if (command.action === "mark-unread")
      c.markUnread(command.id, command.target);
    else if (command.action === "change-project") c.changeProject(command.id);
    else if (command.action === "remove-chat")
      c.remove(command.id, command.room);
    else if (command.action === "add-project") c.addProject();
    else if (command.action === "project-folders")
      c.projectFolders?.(command.projectId);
    else if (command.action === "delete-project")
      c.deleteProject?.(command.projectId);
    else if (command.action === "project-account")
      c.projectAccount(command.path);
    else if (command.action === "settings") c.settings?.();
    else if (command.action === "search") {
      areas.sidebar.close();
      c.onSearch();
    }
  };
}

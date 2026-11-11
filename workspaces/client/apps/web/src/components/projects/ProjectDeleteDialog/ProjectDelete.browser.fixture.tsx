import "../../../rendererStyles.ts";
import { createSidebarSourceStore } from "../../../servers/sidebarSourceFixture.ts";
import type { ComponentSnapshot as Snapshot } from "../../conversation/ComposerAttachments/componentTestFixtures.ts";
import { createProjectionMetadataMirror } from "../../../generated/observableResources.ts";
import { stateFromFixture } from "../../../servers/sidebarSourceFixture.ts";
import { SidebarCommandsProvider } from "../../sidebar/support/commands.ts";
import { CombinedSidebarServicesProvider } from "../../../servers/combinedSidebarServices.ts";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
const injectedStore = observableFixture();
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import { createMergedSidebarSelector } from "../../../servers/mergedSidebar.ts";
import { sidebarSnapshot } from "../../../servers/sidebarSnapshot.ts";
import { StopOutcome } from "../../../generated/api.ts";
import {
  AgentSource as GeneratedApiAgentSource,
  AgentStatus as GeneratedApiAgentStatus,
} from "../../../generated/api.ts";
import type { Agent } from "../../../types.ts";
import type {
  SidebarBackend,
  SidebarServices,
} from "../../sidebar/support/services.tsx";
import type { PostBody, PostResult } from "../../../api.ts";
import {
  fixturePostBody,
  fixturePostResult,
} from "../../../../tests/misc-fixture.ts";
declare global {
  interface Window {
    posts: { path: string; body: PostBody<"/api/projects/delete"> }[];
    finished: string[][];
    suspended: string[];
    fail: boolean;
    retire: () => void;
    notice: string | undefined;
    owner: string | undefined;
  }
}
const rootElement = document.getElementById("root");

import React from "react";
import { action, computed, observable, reaction } from "mobx";
import { observer } from "mobx-react-lite";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import Sidebar from "../../sidebar/Sidebar/Sidebar.fixture.tsx";
import {
  createCombinedSidebarFixture,
  CombinedSidebarFixtureView,
} from "../../../servers/CombinedServerSidebar.fixture.tsx";
import { ProjectDeleteDialog } from "./ProjectDeleteDialog.tsx";
import { ProjectDeletion } from "../../../useProjectDelete.ts";
import "@mantine/core/styles.css";
import "../../../design/layout.css";
import "../../../appearance.css";
import "../../sidebar/Sidebar/sidebar-projects.css";
const project = {
  id: "project",
  name: "Example",
  path: "/project",
  homeServerId: "home",
  projectSeq: 12,
};
const agent: Agent = {
  id: "lead",
  cwd: "/project",
  source: GeneratedApiAgentSource.managed,
  isLead: true,
  name: "Chat",
  status: GeneratedApiAgentStatus.paused,
  projectId: "project",
  projectServerId: "home",
};
// This one-off sidebar fixture omits the server token and unrelated runtime fields.
type ProjectDeleteFixtureSnapshot = Partial<Omit<Snapshot, "runtime">> & {
  runtime: Partial<Snapshot["runtime"]>;
};
const partialInitial: ProjectDeleteFixtureSnapshot = {
  stateDir: "fixture",
  entityWorkspaceId: "workspace",
  entitiesComplete: true,
  threads: [agent],
  chats: [],
  nodes: [agent],
  edges: [],
  runtime: {
    projectSequences: { project: 12 },
    agents: [agent],
    projects: [project],
    rooms: [],
    requests: [],
    tasks: [],
    monitors: [],
    complaints: [],
    rules: [],
    peerTeams: [],
    events: [],
    studioErrors: [],
    work: [],
    projectOrganizationVersion: 1,
    peerTeamsVersion: 1,
  },
};
const initial = partialInitial as Snapshot;

window.posts = [];
window.finished = [];
window.suspended = [];
window.fail = false;
window.retire = () => {};
const store = observable.map<string, string>();
const backend: SidebarBackend = {
  ownerId: "owner",
  storage: {
    getItem: (key) => store.get(key) || null,
    setItem: (key, value) => store.set(key, value),
    removeItem: (key) => store.delete(key),
  },
  saved: <T,>(key: string, value: T): T => {
    // This fixture writes and reads its own saved-state values.
    return (JSON.parse(store.get(key) || "null") as T | null) ?? value;
  },
  save: () => {},
  editPreference: (_key, _before, after) => after,
  post: async (path, body) => {
    const request = fixturePostBody<"/api/projects/delete">(body);
    window.posts.push({ path, body: request });
    if (window.fail) throw new TypeError("response lost");
    const response = {
      id: request.id,
      request_id: request.request_id,
      outcome: "deleted",
      stopped: StopOutcome.confirmed,
      deletedAgentIds: ["lead"],
      deletedRoomIds: [],
    } satisfies PostResult<"/api/projects/delete">;
    return fixturePostResult<typeof path>(response);
  },
};
const services: SidebarServices = {
  cache: backend,
  owner: () => backend,
  byOwner: () => backend,
  serverForChat: () => "owner",
};
const fixtureData = observable.box(
  location.search.includes("empty")
    ? {
        ...initial,
        threads: [],
        nodes: [],
        runtime: { ...initial.runtime, agents: [] },
      }
    : initial,
  { deep: false },
);
window.retire = action(() =>
  fixtureData.set({
    ...initial,
    threads: [],
    nodes: [],
    runtime: { ...initial.runtime, agents: [], projects: [] },
  }),
);
const state = computed(() => stateFromFixture(fixtureData.get()));
const metadata = computed(() => {
  const data = fixtureData.get();
  return createProjectionMetadataMirror({
    workspaceId: data.entityWorkspaceId ?? "",
    ready: true,
    complete: data.entitiesComplete === true,
    projectSequences: data.runtime.projectSequences ?? {},
  }).current;
});
const deletion = new ProjectDeletion({
  get state() {
    return state.get();
  },
  get metadata() {
    return metadata.get();
  },
  workspaceId: "workspace",
  services,
  onSuspend: (scope) => {
    window.suspended = [...scope];
  },
  onResume: () => {
    window.suspended = [];
  },
  onFinish: (ids) => {
    window.finished.push(ids);
    window.suspended = [];
  },
  notify: (message) => {
    window.notice = message;
  },
});
const logical = location.search.includes("logical");
const sources = computed(() => {
  const data = fixtureData.get();
  const source = createSidebarSourceStore();
  source.update(
    "owner",
    sidebarSnapshot(
      stateFromFixture({
        ...data,
        runtime: {
          ...data.runtime,
          projects: data.runtime.projects.map((project) => ({
            ...project,
            ...(location.search.includes("legacy") ? { id: "" } : {}),
          })),
        },
      }),
      [
        ...data.runtime.projects.map((value) => ({
          collection: "project" as const,
          id: value.id,
          value,
        })),
        ...data.threads.map((chat) => ({
          collection: "agentSummary" as const,
          id: chat.id,
          value: agentSummaryFixture(chat.id, {
            name: chat.name || "Chat",
            projectPath: chat.projectPath || chat.cwd || "",
          }),
        })),
      ],
    ),
  );
  return source.sources(["owner"], new Set(["owner"]));
});

const select = createMergedSidebarSelector();
const model = computed(() => select(sources.get()));
const combinedServices: import("../../../servers/combinedSidebarServices.ts").CombinedSidebarServices =
  {
    backends: new Map([["owner", backend]]),
    online: () => true,
    refreshOwner: async () => {
      throw new Error("Unexpected refresh");
    },
    executeCommand: async () => {
      throw new Error("Unexpected command");
    },
    aliases: {},
    send: (owner, command) => {
      window.owner = owner;
      if (command.action === "delete-project") deletion.open(command.projectId);
    },
    applyNavigation: () => {},
    settings: () => {},
    search: () => {},
    notify: (message) => {
      window.notice = message;
    },
  };
const sidebarInput = () => ({
  model: model.get(),
  current: "owner",
  navigation: {},
  mobile: true,
  hidden: false,
  close: () => {},
  notice: "",
  hideOldChatsThreshold: 20,
});
const combined = createCombinedSidebarFixture(sidebarInput(), combinedServices);
const stops = [
  deletion.start(),
  ...(logical
    ? [
        combined.start(),
        reaction(
          () => model.get(),
          () => combined.update(sidebarInput(), combinedServices),
        ),
      ]
    : []),
];
window.addEventListener(
  "pagehide",
  () => {
    stops.reverse().forEach((stop) => stop());
    injectedStore.store.dispose();
  },
  { once: true },
);
const Fixture = observer(function Fixture() {
  const data = fixtureData.get();
  const collections = sources.get()[0]!.collections;
  return React.createElement(
    React.Fragment,
    null,
    logical
      ? React.createElement(
          injectedStore.Provider,
          {},
          React.createElement(
            CombinedSidebarServicesProvider,
            {
              value: combinedServices,
            },
            React.createElement(CombinedSidebarFixtureView, {
              fixture: combined,
            }),
          ),
        )
      : React.createElement(
          injectedStore.Provider,
          {},
          React.createElement(
            SidebarCommandsProvider,
            {
              value: {
                settings: () => {},
                onSearch: () => {},
                open: () => {},
                newChat: () => {},
                addProject: () => {},
                changeProject: () => {},
                projectAccount: () => {},
                rename: async () => {},
                remove: () => {},
                markUnread: () => {},
                deleteProject: (path: string) => deletion.open(path),
              },
            },
            React.createElement(Sidebar, {
              services,
              data,
              opened: "lead",
              collections,
              markingRead: new Set<string>(),
              creating: false,
              mobile: true,
              close: () => {},
            }),
          ),
        ),
    React.createElement(ProjectDeleteDialog, deletion.dialog),
  );
});
createRoot(rootElement!).render(
  React.createElement(
    React.StrictMode,
    null,
    React.createElement(MantineProvider, null, React.createElement(Fixture)),
  ),
);

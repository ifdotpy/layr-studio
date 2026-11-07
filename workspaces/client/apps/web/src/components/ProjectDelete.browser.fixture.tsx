import { agentSummaryFixture } from "../servers/agentSummaryFixture.ts";
import {
  createMergedSidebarSelector,
  createSidebarSourceStore,
} from "../servers/mergedSidebar.ts";
import { sidebarSnapshot } from "../servers/sidebarSnapshot.ts";
import { StopOutcome } from "../generated/api.ts";
import {
  AgentSource as GeneratedApiAgentSource,
  AgentStatus as GeneratedApiAgentStatus,
} from "../generated/api.ts";
import type { Agent, Snapshot } from "../types.ts";
import type { SidebarBackend, SidebarServices } from "./sidebar/services.tsx";
import type { PostBody, PostResult } from "../api.ts";
import {
  fixturePostBody,
  fixturePostResult,
} from "../../tests/misc-fixture.ts";
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

import React, { useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import Sidebar from "../components/Sidebar.tsx";
import CombinedServerSidebar from "../servers/CombinedServerSidebar.tsx";
import { ProjectDeleteDialog } from "../components/ProjectDeleteDialog.tsx";
import { useProjectDelete } from "../useProjectDelete.ts";
import "@mantine/core/styles.css";
import "../design/layout.css";
import "../appearance.css";
import "../components/sidebar-projects.css";
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
const store = new Map<string, string>();
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
  subscribePreference: () => () => {},
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
function Fixture() {
  const [data, setData] = useState(
    location.search.includes("empty")
      ? {
          ...initial,
          threads: [],
          nodes: [],
          runtime: { ...initial.runtime, agents: [] },
        }
      : initial,
  );
  const [, setScope] = useState<ReadonlySet<string>>(new Set());
  window.retire = () =>
    setData({
      ...initial,
      threads: [],
      nodes: [],
      runtime: { ...initial.runtime, agents: [], projects: [] },
    });
  const deletion = useProjectDelete({
    data,
    workspaceId: "workspace",
    services,
    onSuspend: (scope) => {
      window.suspended = [...scope];
      setScope(scope);
    },
    onResume: () => setScope(new Set()),
    onFinish: (ids) => {
      window.finished.push(ids);
      setScope(new Set());
    },
    notify: (message) => {
      window.notice = message;
    },
  });
  const logical = location.search.includes("logical");
  const sources = useMemo(() => {
    const source = createSidebarSourceStore();
    source.update(
      "owner",
      sidebarSnapshot(
        {
          ...data,
          runtime: {
            ...data.runtime,
            projects: data.runtime.projects.map((project) => ({
              ...project,
              ...(location.search.includes("legacy") ? { id: "" } : {}),
            })),
          },
        },
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
  }, [data]);
  const model = useMemo(
    () => createMergedSidebarSelector()(sources),
    [sources],
  );
  const collections = sources[0]!.collections;
  return React.createElement(
    React.Fragment,
    null,
    logical
      ? React.createElement(CombinedServerSidebar, {
          model,
          backends: new Map([["owner", backend]]),
          online: () => true,
          refreshOwner: async () => {
            throw new Error("Unexpected refresh");
          },
          executeCommand: async () => {
            throw new Error("Unexpected command");
          },
          aliases: {},
          current: "owner",
          navigation: {},
          send: (owner, command) => {
            window.owner = owner;
            if (command.action === "delete-project")
              deletion.open(command.projectId);
          },
          applyNavigation: () => {},
          mobile: true,
          hidden: false,
          close: () => {},
          settings: () => {},
          search: () => {},
          notify: (message) => {
            window.notice = message;
          },
          notice: "",
          hideOldChatsThreshold: 20,
        })
      : React.createElement(Sidebar, {
          services,
          data,
          opened: "lead",
          collections,
          markingRead: new Set<string>(),
          creating: false,
          mobile: true,
          settings: () => {},
          onSearch: () => {},
          close: () => {},
          open: () => {},
          newChat: () => {},
          addProject: () => {},
          changeProject: () => {},
          projectAccount: () => {},
          rename: async () => {},
          remove: () => {},
          markUnread: () => {},
          deleteProject: (path: string) => deletion.open(path),
          refresh: async () => {
            throw new Error("Unexpected refresh");
          },
        }),
    React.createElement(ProjectDeleteDialog, deletion.dialog),
  );
}
createRoot(rootElement!).render(
  React.createElement(
    React.StrictMode,
    null,
    React.createElement(MantineProvider, null, React.createElement(Fixture)),
  ),
);

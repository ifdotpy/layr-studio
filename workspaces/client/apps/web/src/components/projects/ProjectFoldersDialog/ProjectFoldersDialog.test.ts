// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import ProjectFoldersDialog from "./ProjectFoldersDialog.tsx";
import { createStateMirror } from "../../../generated/observableResources.ts";
import { createServerStore } from "../../../store/server.ts";
import { createSubscriptions } from "../../../sync/subscriptions/module.ts";
const store = createServerStore(
  createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: () => () => {},
  }),
);

const fixtureStore = Object.assign(Object.create(store), {
  nativeLayrSettings: () => ({
    current: {
      status: "live",
      data: { storeRoot: "/default", configured: true },
    },
  }),
  multiServer: () => ({ current: { status: "loading" } }),
});

const leaf = vi.hoisted(() => ({
  post: vi.fn(async () => ({})),
  resources: {
    watchResourceChanges: (
      _ref: unknown,
      accept: (version: {
        nativeLayrSettings: { storeRoot: string; configured: boolean };
      }) => void,
    ) => {
      accept({
        nativeLayrSettings: { storeRoot: "/default", configured: true },
      });
      return () => {};
    },
  },
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerStore: () => fixtureStore,
    useServerApi: () => ({
      getStreamSession: () => undefined,
      post: leaf.post,
      get: async () => ({ gitHead: "main" }),
      refreshSession: async () => ({ token: "fixture" }),
    }),
    useServerStorage: () => ({
      getItem: () => null,
      setItem: () => {},
      removeItem: () => {},
    }),
    useServerResources: () => leaf.resources,
    useServerFileDialogs: () => ({
      canPickDirectory: () => false,
      pickDirectory: vi.fn(),
    }),
  }),
);
vi.mock("../ProjectPeers/ProjectPeers.tsx", () => ({
  ProjectPeers: () => null,
}));

it("project folders updates the real store and location leaves from one stable generated model", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: () => true,
  }));
  const mirror = createStateMirror();
  const project = { id: "/project", path: "/project", name: "Project" };
  const old = {
    ...project,
    layrStoreRoot: "/old-store",
    layrStoreRootRevision: 1,
    locations: [
      { projectId: project.id, serverId: "home", path: "/old-folder" },
    ],
  };
  mirror.apply([{ collection: "project", id: project.id, value: old }]);
  const selected = mirror.current.projects.get(project.id)!;
  if (!validProject(selected)) throw new Error("Missing project");
  const host = document.createElement("div"),
    root = createRoot(host);
  document.body.append(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(ProjectFoldersDialog, {
            project: selected,
          }),
        ),
      ),
    );
    expect(host.querySelector("input")?.value).toBe("/old-store");
    expect(host.textContent).toContain("/old-folder");
    await act(async () =>
      mirror.apply([
        {
          collection: "project",
          id: project.id,
          value: {
            ...old,
            layrStoreRoot: "/new-store",
            layrStoreRootRevision: 3,
          },
        },
      ]),
    );
    expect(mirror.current.projects.get(project.id)).toBe(selected);
    expect(host.querySelector("input")?.value).toBe("/new-store");
    await act(async () =>
      mirror.apply([
        {
          collection: "project",
          id: project.id,
          value: {
            ...old,
            layrStoreRoot: "/new-store",
            layrStoreRootRevision: 3,
            locations: [
              { projectId: project.id, serverId: "home", path: "/new-folder" },
            ],
          },
        },
      ]),
    );
    expect(host.textContent).toContain("/new-folder");
    await act(async () =>
      [...host.querySelectorAll("button")]
        .find((button) => button.textContent === "Reset")
        ?.click(),
    );
    expect(leaf.post).toHaveBeenCalledWith("/api/projects", {
      action: "set_layr_store_root",
      path: project.id,
      expected_revision: 3,
      layr_store_root: null,
    });
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});

function validProject(
  project: import("../../../app/contracts.ts").ProjectModel,
): project is import("../../../app/contracts.ts").ProjectModel & {
  path: string;
  name: string;
} {
  return typeof project.path === "string" && typeof project.name === "string";
}

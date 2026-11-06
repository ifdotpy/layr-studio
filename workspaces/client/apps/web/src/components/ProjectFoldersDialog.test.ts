import { type ReactElement, type ReactNode } from "react";
import { expect, it, vi } from "vitest";
import type { ServerServices } from "../servers/serverServices";
import { componentSnapshot, required } from "./componentTestFixtures";

const h = vi.hoisted(() => ({
  cursor: 0,
  cells: [] as unknown[],
  effects: [] as Array<() => void>,
  accept: undefined as
    | Parameters<ServerServices["sync"]["subscribeStateProjection"]>[0]
    | undefined,
}));
vi.mock("react", async (original) => ({
  ...(await original<typeof import("react")>()),
  useState: (initial: unknown) => {
    const index = h.cursor++;
    if (!(index in h.cells)) h.cells[index] = initial;
    return [
      h.cells[index],
      (value: unknown) => {
        h.cells[index] =
          typeof value === "function" ? value(h.cells[index]) : value;
      },
    ];
  },
  useEffect: (effect: () => void) => {
    h.effects.push(effect);
  },
}));
vi.mock("../servers/ServerServicesContext", () => ({
  useServerApi: () => ({
    get: vi.fn(async () => ({ items: [], identity: {}, servers: [] })),
  }),
  useServerSync: () => ({
    subscribeStateProjection: (accept: typeof h.accept) => {
      h.accept = accept;
      return () => {};
    },
  }),
}));
vi.mock("./ProjectLocationsSettings", () => ({ default: "locations" }));
vi.mock("./ProjectPeers", () => ({ ProjectPeers: "peers" }));
vi.mock("./ProjectLayrStoreSettings", () => ({
  ProjectLayrStoreSettings: "project-store",
}));
import ProjectFoldersDialog from "./ProjectFoldersDialog";

function find(
  node: ReactNode,
):
  | ReactElement<{
      project: {
        layrStoreRoot?: string | null;
        layrStoreRootRevision?: number | null;
      };
    }>
  | undefined {
  if (Array.isArray(node)) return node.map(find).find(Boolean);
  if (!node || typeof node !== "object" || !("props" in node)) return undefined;
  const element = node as ReactElement<{
    project: {
      layrStoreRoot?: string | null;
      layrStoreRootRevision?: number | null;
    };
    children?: ReactNode;
  }>;
  return element.type === "project-store"
    ? element
    : find(element.props.children);
}

it("updates the store and revision from project stream events", () => {
  const props = {
    project: { id: "/project", path: "/project", name: "Project" },
    saved: async () => {},
  };
  const render = () => {
    h.cursor = 0;
    return ProjectFoldersDialog(props);
  };
  render();
  for (const effect of h.effects.splice(0)) effect();
  required(h.accept)(
    componentSnapshot([], {
      projects: [
        {
          id: "/project",
          path: "/project",
          name: "Project",
          layrStoreRoot: "/new",
          layrStoreRootRevision: 3,
        },
      ],
    }),
  );
  const project = required(find(render())).props.project;
  expect(project.layrStoreRoot).toBe("/new");
  expect(project.layrStoreRootRevision).toBe(3);
});

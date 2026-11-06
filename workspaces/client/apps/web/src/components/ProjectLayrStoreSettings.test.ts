import { MantineProvider } from "@mantine/core";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it, vi } from "vitest";

vi.mock("react", async (original) => ({
  ...(await original<typeof import("react")>()),
  useState: (initial: unknown) => [
    initial === null ? { storeRoot: "/default", configured: true } : initial,
    vi.fn(),
  ],
  useEffect: () => {},
}));
vi.mock("../servers/ServerServicesContext", () => ({
  useServerApi: () => ({ post: vi.fn(), get: vi.fn() }),
  useServerResources: () => ({ watchResourceChanges: vi.fn() }),
  useServerFileDialogs: () => ({
    canPickDirectory: () => false,
    pickDirectory: vi.fn(),
  }),
}));
import { ProjectLayrStoreSettings } from "./ProjectLayrStoreSettings";

it("uses the shared chooser and displays the project store", () => {
  const html = renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(ProjectLayrStoreSettings, {
        project: {
          id: "/project",
          path: "/project",
          name: "Project",
          layrStoreRoot: "/project-store",
        },
        saved: async () => {},
      }),
    ),
  );
  expect(html).toContain("Project store directory");
  expect(html).toContain("/project-store");
  expect(html).toContain("Source: project");
  expect(html).toContain("Choose a folder");
  expect(html).toContain("Reset");
});

it("displays the default when the project has no override", () => {
  const html = renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(ProjectLayrStoreSettings, {
        project: { id: "/project", path: "/project", name: "Project" },
        saved: async () => {},
      }),
    ),
  );
  expect(html).toContain("/default");
  expect(html).toContain("Source: default");
});

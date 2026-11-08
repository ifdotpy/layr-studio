import { SidebarCommandsProvider } from "./sidebar/commands.ts";
import { observableFixture } from "./observableFixture.tsx";
import { ServerStoreProvider } from "../servers/ServerServicesContext.tsx";
import { createElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { afterEach, expect, it, vi } from "vitest";
import { componentSnapshot as snapshotFixture } from "./componentTestFixtures.ts";
import Sidebar from "./Sidebar.fixture";
import { AgentSource } from "../generated/api";
import { agentSummaryFixture } from "../servers/agentSummaryFixture.ts";
import { createStateMirror } from "../generated/observableResources.ts";
import { createSidebarRow, localSidebarIdentity } from "./sidebar/viewModel.ts";
const rowFixture = (
  id: string,
  overrides: Parameters<typeof agentSummaryFixture>[1],
) =>
  createSidebarRow(agentSummaryFixture(id, overrides), {
    ...localSidebarIdentity,
    collections: createStateMirror().current,
  });

// Static markup has no layout, so the measured frame would start as a closed
// drawer. The frame's geometry has its own tests; render its content inline here.
vi.mock("./sidebar/SidebarFrame", async (original) => ({
  ...(await original<typeof import("./sidebar/SidebarFrame")>()),
  SidebarFrame: (p: { children: ReactNode }) =>
    createElement("aside", { id: "sidebar" }, p.children),
  restoreSidebarFocus: () => {},
}));

afterEach(() => vi.unstubAllGlobals());

it("leaves the desktop close action to the workspace header", () => {
  vi.stubGlobal("location", new URL("http://studio.test/"));
  vi.stubGlobal("localStorage", { getItem: () => null });
  vi.stubGlobal("window", { matchMedia: () => ({ matches: false }) });
  const markup = renderToStaticMarkup(
    createElement(
      ServerStoreProvider,
      { value: observableFixture().store },
      createElement(
        MantineProvider,
        null,
        createElement(
          SidebarCommandsProvider,
          {
            value: {
              onSearch: () => {},
              open: () => {},
              newChat: () => {},
              addProject: () => {},
              changeProject: () => {},
              projectAccount: () => {},
              rename: async () => {},
              remove: () => {},
              markUnread: () => {},
              refresh: async () => {},
            },
          },
          createElement(Sidebar, {
            data: Object.assign(snapshotFixture(), {
              token: "",
              stateDir: "/fixture",
            }),
            opened: "",
            rows: [],
            markingRead: new Set<string>(),
            creating: false,
            mobile: false,
            close: () => {},
          }),
        ),
      ),
    ),
  );
  expect(markup).toContain('aria-label="Search chats"');
  expect(markup).not.toContain('aria-label="Close conversations"');
});

it("shows an upgraded chat at the project root without changing its old line folder", () => {
  vi.stubGlobal("location", new URL("http://studio.test/"));
  vi.stubGlobal("localStorage", { getItem: () => null });
  vi.stubGlobal("window", { matchMedia: () => ({ matches: false }) });
  const line = "/store/projects/id/lines/main";
  const folders = [{ id: "old-folder", name: "Old folder" }];
  const data = Object.assign(
    snapshotFixture(
      [
        {
          id: "lead",
          name: "Upgraded chat",
          source: AgentSource.managed,
          isLead: true,
          cwd: line,
          projectPath: "/source",
          projectFolder: "old-folder",
        },
      ],
      {
        projects: [
          { id: "/source", path: "/source", name: "Source", folders: [] },
          { id: line, path: line, name: "main", folders },
        ],
      },
    ),
    { token: "", stateDir: "/fixture" },
  );
  const before = JSON.stringify(data.runtime.projects);
  const markup = renderToStaticMarkup(
    createElement(
      ServerStoreProvider,
      { value: observableFixture().store },
      createElement(
        MantineProvider,
        null,
        createElement(
          SidebarCommandsProvider,
          {
            value: {
              onSearch: () => {},
              open: () => {},
              newChat: () => {},
              addProject: () => {},
              changeProject: () => {},
              projectAccount: () => {},
              rename: async () => {},
              remove: () => {},
              markUnread: () => {},
              refresh: async () => {},
            },
          },
          createElement(Sidebar, {
            data,
            opened: "",
            rows: [
              rowFixture("lead", {
                name: "Upgraded chat",
                projectPath: "/source",
                projectFolder: "old-folder",
              }),
            ],
            markingRead: new Set<string>(),
            creating: false,
            mobile: false,
            close: () => {},
          }),
        ),
      ),
    ),
  );
  expect(markup.match(/data-chat="lead"/g)).toHaveLength(1);
  const sections: string[] = [];
  let ancestors: string[] | undefined;
  for (const tag of markup.matchAll(
    /<section\b[^>]*>|<\/section>|<[^>]*data-chat="lead"[^>]*>/g,
  )) {
    if (tag[0] === "</section>") sections.pop();
    else if (tag[0].startsWith("<section")) sections.push(tag[0]);
    else ancestors = [...sections];
  }
  expect(
    ancestors?.some((tag) => tag.includes('data-project-path="/source"')),
  ).toBe(true);
  expect(ancestors?.some((tag) => tag.includes("data-folder-id="))).toBe(false);
  expect(JSON.stringify(data.runtime.projects)).toBe(before);
});

import { createElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { afterEach, expect, it, vi } from "vitest";
import { snapshotFixture } from "../snapshotFixture";
import Sidebar from "./Sidebar";
import { AgentSource } from "../generated/api";
import { sidebarRowFixture } from "../servers/sidebarRowFixture.ts";

// Static markup has no layout, so the measured frame would start as a closed
// drawer. The frame's geometry has its own tests; render its content inline here.
vi.mock("./sidebar/SidebarFrame", () => ({
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
      MantineProvider,
      null,
      createElement(Sidebar, {
        data: snapshotFixture(),
        opened: "",
        rows: [],
        markingRead: new Set<string>(),
        creating: false,
        mobile: false,
        close: () => {},
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
      }),
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
  const data = snapshotFixture(
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
  );
  const before = JSON.stringify(data.runtime.projects);
  const markup = renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(Sidebar, {
        data,
        opened: "",
        rows: [
          sidebarRowFixture("lead", {
            name: "Upgraded chat",
            projectPath: "/source",
            projectFolder: "old-folder",
          }),
        ],
        markingRead: new Set<string>(),
        creating: false,
        mobile: false,
        close: () => {},
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
      }),
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

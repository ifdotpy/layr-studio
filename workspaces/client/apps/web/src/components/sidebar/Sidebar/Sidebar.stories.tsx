import { observable, action } from "mobx";
import {
  SidebarCommandsProvider,
  type SidebarCommands,
} from "../support/commands.ts";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import type { components } from "../../../generated/api.ts";
import { useEffect, useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMantineColorScheme } from "@mantine/core";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import { publishServerAliases } from "../../../servers/serverAliases.ts";
import { readServerAliases } from "../../../servers/registry.ts";
import Sidebar, { type SidebarProps } from "./Sidebar.fixture.tsx";
import { createStateMirror } from "../../../generated/observableResources.ts";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import {
  createSidebarServices,
  type SidebarBackend,
} from "../support/services.tsx";
import "../support/FleetFixtures.css";

const now = Date.now() / 1000;
const projects = ["codex-studio", "atlas-api", "orchard-web"];
const titles = [
  "Review deployment permissions",
  "Repair failing database migration",
  "Обновление интерфейса нескольких параллельных проектов",
  "Document the release checklist and rollback procedure",
  "Investigate mobile navigation",
  "Ready for the next assignment",
];
const statuses: components["schemas"]["AgentOperationalStatus"][] = [
  { reason: "needs_input", wait: null },
  { reason: "failed" },
  { reason: "working" },
  { reason: "read_result" },
  { reason: "ready" },
  { reason: "ready" },
];
const summaries = projects.flatMap((project, projectIndex) =>
  titles.map((name, index) =>
    agentSummaryFixture(`${project}-${index}`, {
      name,
      projectPath: `/work/${project}`,
      cwd: `/work/${project}`,
      updated: now - (projectIndex + index + 1) * 180,
      pinned: projectIndex === 0 && index === 2,
      provider: index % 2 ? "claude" : "codex",
      model: index % 2 ? "claude-opus-4-6" : "gpt-5.4",
      operationalStatus: statuses[index]!,
      inFlight: index === 2,
      markUnread: "no_completed_result",
    }),
  ),
);
const projectRows = projects.map((name) => ({
  id: name,
  name,
  path: `/work/${name}`,
  created: now - 86400,
}));
function storyCollections(
  agents: components["schemas"]["AgentSummaryEntityDto"][],
) {
  const mirror = createStateMirror();
  mirror.apply([
    ...agents.map((value) => ({
      collection: "agentSummary" as const,
      id: value.id,
      value,
    })),
    ...projectRows.map((value) => ({
      collection: "project" as const,
      id: value.id,
      value,
    })),
    {
      collection: "workspace",
      id: "fixture",
      value: {
        stateDir: "/fixture",
        peerTeamsVersion: 1,
        projectOrganizationVersion: 1,
      },
    },
  ]);
  return mirror.current;
}
const values = observable.map<string, string>();
const backend: SidebarBackend = {
  ownerId: "local",
  post: async () => {
    throw new Error("Story fixture does not write to a server.");
  },
  saved: (key, fallback) => JSON.parse(values.get(key) || "null") ?? fallback,
  save: (key, value) => {
    values.set(key, JSON.stringify(value));
  },
  storage: {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => {
      values.set(key, value);
    },
    removeItem: (key) => {
      values.delete(key);
    },
  },
  editPreference: action((key, _before, after) => {
    values.set(key, JSON.stringify(after));
    return after;
  }),
};
const services = createSidebarServices(
  backend,
  [backend, { ...backend, ownerId: "remote" }],
  (target) =>
    target.kind === "chat" && target.id === "atlas-api-1" ? "remote" : "local",
);
services.alias = (target) =>
  target.kind === "chat" && target.id === "atlas-api-1" ? "BLD" : "MAC";
const args: SidebarProps & SidebarCommands = {
  services,
  collections: storyCollections(summaries),
  opened: "codex-studio-2",
  open: fn(),
  newChat: fn(),
  newSharedChat: fn(),
  addProject: fn(),
  changeProject: fn(),
  projectAccount: fn(),
  creating: false,
  rename: fn(),
  remove: fn(),
  mobile: true,
  onSearch: fn(),
  close: fn(),
  markUnread: fn(),
  markingRead: new Set(),
  settings: fn(),
};
function FleetFixture({
  scheme,
  ...props
}: SidebarProps & SidebarCommands & { scheme: "light" | "dark" }) {
  const [opened, open] = useState(props.opened);
  const { setColorScheme } = useMantineColorScheme();
  useEffect(() => setColorScheme(scheme), [scheme, setColorScheme]);
  return (
    <SidebarCommandsProvider
      value={{
        ...props,
        open: (id) => {
          open(id);
          props.open(id);
        },
      }}
    >
      <div id="root" className="fleet-story">
        <Sidebar {...props} opened={opened} />
      </div>
    </SidebarCommandsProvider>
  );
}
const meta: Meta<SidebarProps & SidebarCommands> = {
  title: "Sidebar/Fleet board",
  component: Sidebar,
  decorators: [
    (Story) => {
      const [fixture] = useState(() => observableFixture());
      return (
        <fixture.Provider>
          <SidebarCommandsProvider value={args}>
            <Story />
          </SidebarCommandsProvider>
        </fixture.Provider>
      );
    },
  ],
  args,
  parameters: { layout: "fullscreen" },
  beforeEach: () => {
    values.clear();
    const previous = readServerAliases();
    publishServerAliases({ local: "MAC" });
    return () => publishServerAliases(previous);
  },
};
export default meta;
type Story = StoryObj<typeof meta>;
export const Dark: Story = {
  render: (props) => <FleetFixture {...props} scheme="dark" />,
};
export const Light: Story = {
  render: (props) => <FleetFixture {...props} scheme="light" />,
};
export const CompactHeader: Story = {
  render: (props) => <FleetFixture {...props} scheme="dark" />,
  play: async ({ canvasElement, args }) => {
    const sidebar = canvasElement.querySelector("#sidebar")!;
    await expect(sidebar.querySelector(".sidebar-nav")).toBeNull();
    await expect(sidebar.querySelector("kbd")).toBeNull();
    const header = within(
      sidebar.querySelector<HTMLElement>(".sidebar-header")!,
    );
    const search = header.getByRole("button", { name: "Search chats" });
    await expect(search).toHaveAttribute(
      "aria-keyshortcuts",
      "Control+k Meta+k",
    );
    await expect(search).toHaveAttribute(
      "title",
      expect.stringContaining("Ctrl+K"),
    );
    await userEvent.click(search);
    await expect(args.onSearch).toHaveBeenCalledOnce();
    const projects = within(
      sidebar.querySelector<HTMLElement>(".projects-heading")!,
    );
    await userEvent.click(projects.getByRole("button", { name: "New chat" }));
    await expect(args.newChat).toHaveBeenCalledWith();
    await userEvent.click(
      projects.getByRole("button", { name: "New shared chat" }),
    );
    await expect(args.newSharedChat).toHaveBeenCalledWith();
    await userEvent.click(
      projects.getByRole("button", { name: "Add project" }),
    );
    await expect(args.addProject).toHaveBeenCalledOnce();
    await expect(
      within(sidebar as HTMLElement).getByRole("searchbox", {
        name: "Filter projects and chats",
      }),
    ).toBeVisible();
  },
};
export const Interactions: Story = {
  render: (props) => <FleetFixture {...props} scheme="dark" />,
  play: async ({ canvasElement, args }) => {
    const page = within(canvasElement);
    const lane = page.getByRole("region", { name: "Needs you" });
    for (const strip of canvasElement.querySelectorAll(".fleet-strip")) {
      if (strip.getAttribute("data-sidebar-item") === "atlas-api-1") {
        await expect(
          strip.querySelector(".fleet-server-badge"),
        ).toHaveTextContent("BLD");
      } else {
        await expect(strip.querySelector(".fleet-server-badge")).toBeNull();
      }
      await expect(
        strip.querySelector(".fleet-chat-heading .fleet-server-badge"),
      ).toBeNull();
    }
    const firstStrip = lane.querySelector(".fleet-strip")!;
    const answerStatus = firstStrip.querySelector(
      '[data-chat-status="answer"]',
    )!;
    await expect(
      answerStatus.closest(".mantine-VisuallyHidden-root"),
    ).toBeNull();
    const workingStatus = canvasElement.querySelector(
      '[data-chat-status="working"]',
    )!;
    await expect(
      workingStatus.closest(".mantine-VisuallyHidden-root"),
    ).toBeNull();
    await expect(getComputedStyle(firstStrip).boxShadow).toBe("none");
    // sidebar-projects.css applies a background only to hovered or selected rows.
    await expect(getComputedStyle(firstStrip).backgroundColor).toBe(
      "rgba(0, 0, 0, 0)",
    );
    await expect(page.queryByLabelText("Elapsed time")).not.toBeInTheDocument();
    await expect(
      canvasElement.querySelector(".fleet-chat-provider"),
    ).toBeNull();
    await expect(canvasElement.querySelector(".fleet-chat-time")).toBeNull();
    await expect(
      [...lane.querySelectorAll(".fleet-strip")].map((row) =>
        row.getAttribute("data-studio-state"),
      ),
    ).toEqual(["answer", "answer", "answer", "error", "error", "error"]);
    await expect(
      page.getAllByRole("img", { name: "1 working, 1 needs you, 1 failed" }),
    ).toHaveLength(3);
    const [review] = within(lane).getAllByRole("button", {
      name: /Review deployment permissions/,
    });
    if (!review) throw new Error("Missing review deployment button");
    await userEvent.click(review);
    await expect(args.open).toHaveBeenCalledWith("codex-studio-0");
    await expect(page.getByRole("navigation", { name: "Chats" })).toBeVisible();
    await userEvent.type(
      page.getByRole("searchbox", { name: "Filter projects and chats" }),
      "database",
    );
    await expect(
      page.getAllByText("Repair failing database migration").length,
    ).toBeGreaterThan(0);
    await expect(
      page.queryByText("Investigate mobile navigation"),
    ).not.toBeInTheDocument();
    await userEvent.clear(
      page.getByRole("searchbox", { name: "Filter projects and chats" }),
    );
    const [actions] = page.getAllByRole("button", {
      name: "Actions for Review deployment permissions",
    });
    if (!actions) throw new Error("Missing review deployment actions");
    await userEvent.click(actions);
    const unread = await within(document.body).findByRole("menuitem", {
      name: "Mark unread",
    });
    await waitFor(() => expect(unread).toBeVisible());
    await expect(unread).toBeDisabled();
    await userEvent.click(
      within(document.body).getByRole("menuitem", { name: "Rename" }),
    );
    await expect(
      page.getAllByRole("textbox", { name: "Chat name" }),
    ).toHaveLength(1);
    await expect(
      within(lane).getByRole("textbox", { name: "Chat name" }),
    ).toBeVisible();
    await userEvent.click(page.getByRole("button", { name: /^Cancel$/ }));
  },
};

export const NoAttention: Story = {
  args: {
    collections: storyCollections(
      summaries.map((agent) => ({
        ...agent,
        operationalStatus: { reason: "ready" },
      })),
    ),
  },
  render: (props) => <FleetFixture {...props} scheme="light" />,
  play: async ({ canvasElement }) => {
    await expect(
      within(canvasElement).queryByRole("region", { name: "Needs you" }),
    ).not.toBeInTheDocument();
  },
};

export const Offline: Story = {
  args: { offlineServers: new Set(["local", "remote"]) },
  render: (props) => <FleetFixture {...props} scheme="dark" />,
  play: async ({ canvasElement }) => {
    const badges = canvasElement.querySelectorAll<HTMLElement>(
      ".fleet-server-badge",
    );
    await expect(badges.length).toBeGreaterThan(0);
    for (const badge of badges) {
      await expect(badge).toHaveAttribute("data-offline", "true");
      await expect(badge).toHaveAccessibleName(/unreachable/);
      await expect(badge.querySelector("svg")).toBeVisible();
      const color = document.createElement("span");
      color.style.color = "var(--studio-state-error)";
      badge.append(color);
      await expect(getComputedStyle(badge).color).toBe(
        getComputedStyle(color).color,
      );
      color.remove();
    }
  },
};

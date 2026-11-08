// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { observableFixture } from "../../integration-support/observableFixture.tsx";
import {
  PeerTeamActionsProvider,
  type PeerTeamActions,
} from "../PeerTeamGroup/PeerTeamGroup.tsx";
import { MantineProvider, type CheckboxProps } from "@mantine/core";
import { agentSummaryFixture } from "../../../../servers/agentSummaryFixture.ts";
import { createStateMirror } from "../../../../generated/observableResources.ts";
import {
  createSidebarRow,
  localSidebarIdentity,
} from "../../../sidebar/support/viewModel.ts";
const rowFixture = (
  id: string,
  overrides: Parameters<typeof agentSummaryFixture>[1],
) =>
  createSidebarRow(agentSummaryFixture(id, overrides), {
    ...localSidebarIdentity,
    collections: createStateMirror().current,
  });
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PeerLeadRows, PeerTeamForm } from "../PeerTeamGroup/PeerTeamGroup.tsx";

const candidateRenders = vi.hoisted(() => [] as string[]);
vi.mock("@mantine/core", async (original) => {
  const actual = await original<typeof import("@mantine/core")>();
  return {
    ...actual,
    Checkbox: (props: CheckboxProps) => {
      candidateRenders.push(String(props.label));
      return createElement(actual.Checkbox, props);
    },
  };
});

it("uses the remote candidate facade instead of the local server store", async () => {
  const local = observableFixture();
  const remote = createStateMirror();
  remote.apply([
    {
      collection: "agentSummary",
      id: "sibling",
      value: agentSummaryFixture("sibling", {
        name: "Remote sibling",
        projectPath: "/source",
      }),
    },
    {
      collection: "agentSummary",
      id: "remote",
      value: agentSummaryFixture("remote", {
        name: "Remote candidate",
        projectPath: "/source",
      }),
    },
  ]);
  local.apply([
    {
      collection: "agentSummary",
      id: "local",
      value: agentSummaryFixture("local", {
        name: "Local candidate",
        projectPath: "/source",
      }),
    },
  ]);
  const area: PeerTeamActions = {
    scope: "remote",
    openRoom: () => {},
    closeForm: () => {},
    edit: () => {},
    toggle: () => {},
    room: () => undefined,
    disabled: () => false,
    candidates: () =>
      [...remote.current.agents.values()].map((agent) =>
        createSidebarRow(agent, {
          ...localSidebarIdentity,
          collections: remote.current,
        }),
      ),
  };
  const host = document.createElement("div");
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          local.Provider,
          null,
          createElement(
            MantineProvider,
            null,
            createElement(
              PeerTeamActionsProvider,
              { value: area },
              createElement(PeerTeamForm, {
                project: { id: "/source", path: "/source" },
                action: "save",
              }),
            ),
          ),
        ),
      ),
    );
    expect(host.textContent).toContain("Remote candidate");
    expect(host.textContent).not.toContain("Local candidate");
    candidateRenders.length = 0;
    await act(async () =>
      remote.apply([
        {
          collection: "agentSummary",
          id: "remote",
          value: agentSummaryFixture("remote", {
            name: "Renamed remote",
            projectPath: "/source",
          }),
        },
      ]),
    );
    expect(candidateRenders).toEqual(["Renamed remote"]);
  } finally {
    await act(async () => root.unmount());
    local.store.dispose();
  }
});

const area: PeerTeamActions = {
  scope: "fixture",
  openRoom: () => {},
  closeForm: () => {},
  edit: () => {},
  toggle: () => {},
  room: () => undefined,
  disabled: () => false,
  candidates: () => [
    rowFixture("lead", { name: "Line-backed lead", projectPath: "/source" }),
  ],
};
beforeEach(() => {
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
});
afterEach(() => vi.unstubAllGlobals());
const render = (view: React.ReactNode) =>
  renderToStaticMarkup(
    createElement(PeerTeamActionsProvider, { value: area }, view),
  );
describe("peer leads in a team", () => {
  it("offers line-backed chats by their project path", () => {
    const html = render(
      createElement(
        MantineProvider,
        null,
        createElement(PeerTeamForm, {
          project: { id: "/source", path: "/source" },
          action: "save",
        }),
      ),
    );
    expect(html).toContain("Line-backed lead");
  });
  it("shows the server label, a peer marker and a message field", () => {
    const html = render(
      createElement(PeerLeadRows, {
        team: {
          id: "team",
          peerLeads: [
            {
              grant: "grant-1",
              remoteProject: "project-2",
              server: "server-2",
              label: "Laptop",
            },
          ],
        },
      }),
    );
    expect(html).toContain('data-peer-lead="grant-1"');
    expect(html).toContain("Laptop");
    expect(html).toContain("Peer");
    expect(html).toContain('aria-label="Message Laptop"');
    expect(html).toContain("Send");
  });

  it("renders nothing when the team has no peer leads", () => {
    expect(
      renderToStaticMarkup(
        createElement(PeerLeadRows, { team: { id: "empty" } }),
      ),
    ).toBe("");
    expect(
      renderToStaticMarkup(
        createElement(PeerLeadRows, { team: { id: "empty", peerLeads: [] } }),
      ),
    ).toBe("");
  });
});

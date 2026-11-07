import { createElement } from "react";
import { MantineProvider } from "@mantine/core";
import { sidebarRowFixture } from "../../../servers/sidebarRowFixture.ts";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PeerLeadRows, PeerTeamForm } from "./PeerTeams";

describe("peer leads in a team", () => {
  it("offers line-backed chats by their project path", () => {
    const html = renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(PeerTeamForm, {
          project: { id: "/source", path: "/source" },
          teams: [],
          action: "save",
          close: () => {},
          agents: [
            sidebarRowFixture("lead", {
              name: "Line-backed lead",
              projectPath: "/source",
            }),
          ],
        }),
      ),
    );
    expect(html).toContain("Line-backed lead");
  });
  it("shows the server label, a peer marker and a message field", () => {
    const html = renderToStaticMarkup(
      createElement(PeerLeadRows, {
        leads: [
          {
            grant: "grant-1",
            remoteProject: "project-2",
            server: "server-2",
            label: "Laptop",
          },
        ],
        send: () => Promise.resolve(),
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
      renderToStaticMarkup(createElement(PeerLeadRows, { leads: undefined })),
    ).toBe("");
    expect(
      renderToStaticMarkup(createElement(PeerLeadRows, { leads: [] })),
    ).toBe("");
  });
});

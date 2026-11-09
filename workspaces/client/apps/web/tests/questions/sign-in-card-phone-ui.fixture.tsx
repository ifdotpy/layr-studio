import "../../src/rendererStyles.ts";
import { RequestFixture } from "../../src/components/conversation/questions/support/requestFixtures.ts";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
declare global {
  interface Window {
    __opened: string[];
    __copied: string[];
    __waiting: () => void;
  }
}
import { signInCards, phoneSignInUrl } from "./sign-in.fixture.ts";

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import "../../src/studio-theme.css";
import Requests, {
  RequestArea,
} from "../../src/components/conversation/questions/Requests/Requests.tsx";
const agent = {
  id: "worker",
  rootId: "lead",
  name: "Cluster worker with a long chat name",
  epoch: 0,
  accountKey: "default",
};
const now = Date.now() / 1000;
const signInCard = signInCards({
  clientId: "cli-0123456789abcdef",
  scope: "sso:account:access openid profile",
  source: "host",
  expiresAt: now + 540,
});
let requests = [
  signInCard("redirect", phoneSignInUrl, "id.example.com", {
    host: "127.0.0.1",
    port: 40645,
    path: "/oauth/callback",
  }),
  signInCard(
    "device",
    "https://device.sso.eu-central-1.amazonaws.com/?user_code=WDJB-MJHT",
    "device.sso.eu-central-1.amazonaws.com",
    null,
    { userCode: "WDJB-MJHT" },
  ),
];

window.__notes = [];
const rootNode = createRoot(document.getElementById("root")!);
const render = () =>
  rootNode.render(
    React.createElement(
      MantineProvider,
      null,
      React.createElement(
        "div",
        { style: { padding: "0 16px" } },
        React.createElement(
          RequestFixture,
          {
            requests,
            agents: [agentSummaryFixture(agent.id, agent)],
            details: [{ id: agent.id }],
          },
          React.createElement(
            RequestArea,
            {
              scope: "fixture",
              notify: (s: string) => window.__notes.push(s),
            },
            React.createElement(Requests, { all: true }),
          ),
        ),
      ),
    ),
  );
// The server keeps the card pending in the waiting phase after a paste-mode approval.
window.__waiting = () => {
  requests = requests.map((r) =>
    r.id === "redirect" && r.request.kind === "browser_open"
      ? {
          ...r,
          request: {
            ...r.request,
            phase: { kind: "waiting" as const, expires_at: now + 600 },
          },
        }
      : r,
  );
  render();
};
render();

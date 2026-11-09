import "../../src/rendererStyles.ts";
import { RequestFixture } from "../../src/components/conversation/questions/support/requestFixtures.ts";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
import type { DesktopBridge } from "../../src/desktop.ts";
declare global {
  interface Window {
    // __opened is a string[] in sign-in-card-phone-ui.fixture.tsx.
    browserApprovalOpened: Array<
      [string, Parameters<NonNullable<DesktopBridge["openSignIn"]>>[0]]
    >;
  }
}
import { signInCards, browserSignInUrl } from "./sign-in.fixture.ts";

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import Requests, {
  RequestArea,
} from "../../src/components/conversation/questions/Requests/Requests.tsx";
const agent = {
  id: "worker",
  rootId: "lead",
  name: "Cluster worker",
  epoch: 0,
  accountKey: "default",
};
const signInCard = signInCards({
  clientId: "kube",
  scope: "openid",
});
const requests = [
  signInCard("approve", browserSignInUrl, "accounts.example.com", {
    host: "localhost",
    port: 8000,
    path: "/",
  }),
  signInCard(
    "remote",
    "https://oidc.eu-central-1.amazonaws.com/authorize?x=1",
    "oidc.eu-central-1.amazonaws.com",
    { host: "127.0.0.1", port: 40645, path: "/oauth/callback" },
  ),
  signInCard(
    "device",
    "https://device.sso.eu-central-1.amazonaws.com/?user_code=ABCD",
    "device.sso.eu-central-1.amazonaws.com",
    null,
    { userCode: "ABCD" },
  ),
];

window.__notes = [];
createRoot(document.getElementById("root")!).render(
  React.createElement(
    MantineProvider,
    null,
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
);

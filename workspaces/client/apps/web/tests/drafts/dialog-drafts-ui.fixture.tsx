import "../../src/rendererStyles.ts";
import {
  RequestFixture,
  createRequestFixture,
} from "../../src/components/conversation/questions/support/requestFixtures.ts";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
import type { Agent, Complaint } from "../../src/types.ts";
import { approval } from "../../src/approvalFixtures.ts";
declare global {
  interface Window {
    notices: string[];
    restoreStorage: () => void;
  }
}

import { messageModels } from "../messages/message-models.fixture";
import { conversationActionsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import React, { useEffect, useLayoutEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import Picker from "../../src/components/projects/ProjectDirectoryPicker/ProjectDirectoryPicker.tsx";
import UserMessages from "../../src/components/shell/messages/UserMessages/UserMessages.tsx";
import MessageDate from "../../src/components/conversation/transcript/MessageDate/MessageDate.tsx";
import Requests from "../../src/components/conversation/questions/Requests/Requests.tsx";
const lead = {
  id: "lead",
  rootId: "lead",
  name: "Lead",
  isLead: true,
  source: "managed",
} satisfies Agent;
const complaint = {
  id: "complaint",
  leadId: "lead",
  author: "lead",
  authorName: "Lead",
  recipient: "user",
  title: "Help needed",
  status: "open",
  needsUserResponse: true,
  created: 1,
} satisfies Complaint;
const request = approval({
  id: "question",
  agent: "lead",
  request: {
    kind: "question",
    questions: [
      {
        id: "public",
        question: "Public answer",
        header: null,
        options: [],
        is_secret: false,
        multi_select: null,
        is_other: null,
      },
      {
        id: "secret",
        question: "Private answer",
        header: null,
        options: [],
        is_secret: true,
        multi_select: null,
        is_other: null,
      },
    ],
  },
});
const requestOwners = new Map(
  ["workspace-a", "workspace-b"].map((scope) => {
    const fixture = createRequestFixture({
      scope,
      notify: (message) => window.notices.push(message),
      requests: [request],
      agents: [agentSummaryFixture(lead.id, lead)],
      details: [{ id: lead.id }],
    });
    fixture.start();
    return [scope, fixture] as const;
  }),
);
window.addEventListener(
  "pagehide",
  () => requestOwners.forEach((owner) => owner.dispose()),
  { once: true },
);
function Fixture() {
  const [show, setShow] = useState(false),
    [workspace, setWorkspace] = useState("workspace-a");
  const models = useMemo(() => messageModels(workspace, [lead]), [workspace]);
  useLayoutEffect(
    () =>
      models.apply([
        { collection: "complaint", id: complaint.id, value: complaint },
      ]),
    [models],
  );
  useEffect(() => () => models.close(), [models]);
  return (
    <MantineProvider>
      <button onClick={() => setShow(!show)}>Toggle messages</button>
      <button
        onClick={() =>
          setWorkspace(
            workspace === "workspace-a" ? "workspace-b" : "workspace-a",
          )
        }
      >
        Switch workspace
      </button>
      <div data-missing-date>
        <MessageDate at={0} />
      </div>
      <RequestFixture owner={requestOwners.get(workspace)!}>
        <Requests all />
      </RequestFixture>
      <Picker initialPath="/project" onSelect={async () => {}} />
      {show && (
        <models.Provider commands={conversationActionsFixture()}>
          <UserMessages />
        </models.Provider>
      )}
    </MantineProvider>
  );
}
window.notices = [];
createRoot(document.getElementById("root")!).render(<Fixture />);

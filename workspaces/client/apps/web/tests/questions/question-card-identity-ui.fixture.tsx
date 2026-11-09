import "../../src/rendererStyles.ts";
import { AgentModelsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { ComponentProps } from "react";
import { observer } from "mobx-react-lite";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { requestFixture } from "../../src/components/conversation/questions/support/requestFixtures.ts";
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import type { Message } from "../../src/types.ts";
import {
  approval,
  question as questionField,
} from "../../src/approvalFixtures.ts";
import type { Approval } from "../../src/approvals.ts";
import Requests, {
  RequestCard,
  RequestArea,
} from "../../src/components/conversation/questions/Requests/Requests.tsx";
import History from "../../src/components/conversation/transcript/TurnHistory/TurnHistory.fixture.tsx";
function TurnHistory(props: ComponentProps<typeof History>) {
  return (
    <AgentModelsFixture agents={props.agent ? [props.agent] : []}>
      <History {...props} />
    </AgentModelsFixture>
  );
}
import {
  questionCard,
  transcriptRequests,
  type QuestionLookup,
} from "../../src/components/conversation/support/questionCardModel.ts";

const question: Message = {
  id: "lead:question-item",
  role: "output",
  toolStatus: "running",
  text: JSON.stringify({
    id: "question-item",
    type: "mcpToolCall",
    tool: "AskUserQuestion",
    status: "inProgress",
    arguments: {
      questions: [{ question: "Continue?", options: [{ label: "Yes" }] }],
    },
  }),
};
const approvalItem: Message = {
  id: "lead:command-item",
  role: "output",
  toolStatus: "running",
  text: JSON.stringify({
    id: "command-item",
    type: "commandExecution",
    command: "pwd",
    status: "inProgress",
  }),
};
const initial: Approval[] = [
  approval({
    id: "question",
    agent: "lead",
    request: {
      kind: "question",
      questions: [questionField("0", "Continue?", ["Yes"])],
    },
    params: { itemId: "question-item" },
  }),
  approval({
    id: "approval",
    agent: "lead",
    request: { kind: "exec", command: "pwd", cwd: "/", reason: null },
    params: { itemId: "command-item" },
  }),
];
const Fixture = observer(function Fixture() {
  const [fixture] = useState(() => requestFixture(initial));
  const requests = [...fixture.live.current.requests.values()];
  const [items, setItems] = useState([question, approvalItem]);
  window.publishAnsweredRequest = (id: string) => {
    const request = requests.find((entry) => entry.id === id);
    if (!request) throw new Error("Missing fixture request");

    fixture.live.apply(
      requests
        .filter((entry) => entry.id !== request.id)
        .map((value) => ({
          collection: "request",
          id: value.id,
          value,
        })),
    );
    if (request.id === "question")
      setItems((current) =>
        current.map((item) =>
          item.id === question.id
            ? {
                ...question,
                toolStatus: "completed",
                text: JSON.stringify({
                  id: "question-item",
                  type: "mcpToolCall",
                  tool: "AskUserQuestion",
                  status: "completed",
                  arguments: {
                    questions: [
                      {
                        question: "Continue?",
                        options: [{ label: "Yes" }],
                      },
                    ],
                  },
                  result: {
                    content: 'User has answered: "Continue?"="Yes". Continue.',
                  },
                }),
              }
            : item,
        ),
      );
  };
  const lookup: QuestionLookup = {
    agentId: "lead",
    history: new Map(),
    pending: new Map([["lead\u0000question-item", "question"]]),
    pendingIds: new Set(requests.map((request) => request.id)),
  };
  const cards = (item: Message) => questionCard(item, lookup);
  const linked = transcriptRequests(items, requests, cards);
  const ids = new Set(
    [...linked.values()].flatMap((request) => (request ? [request.id] : [])),
  );
  const notify = (text: string) => {
    throw new Error(text);
  };
  return (
    <ServerStoreProvider value={fixture.store}>
      <RequestArea scope="identity" notify={notify}>
        <main id="messages" style={{ maxWidth: 650, margin: "24px auto" }}>
          <TurnHistory
            items={items}
            enabled={false}
            storageKey="question-identity"
            questionCards={cards}
            requestCards={
              new Map(
                [...linked].map(([messageId, request]) => [
                  messageId,
                  request && (
                    <RequestArea
                      key={request.id}
                      scope="identity"
                      notify={notify}
                    >
                      <RequestCard request={request} messageId={messageId} />
                    </RequestArea>
                  ),
                ]),
              )
            }
            renderMessage={(item) => <p>{item.text}</p>}
            onJump={() => {}}
          />
          <Requests all excludeIds={ids} />
        </main>
      </RequestArea>
    </ServerStoreProvider>
  );
});
const root = document.getElementById("root");
if (!root) throw new Error("Missing fixture root");
createRoot(root).render(
  <MantineProvider>
    <Fixture />
  </MantineProvider>,
);

declare global {
  interface Window {
    publishAnsweredRequest(id: string): void;
  }
}

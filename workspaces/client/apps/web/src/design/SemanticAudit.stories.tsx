import { AgentSource, AgentStatus } from "../generated/api.ts";
import { approval, question, browserOpen } from "../approvalFixtures.ts";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "@mantine/core";
import QuestionCard from "../components/conversation/questions/QuestionCard/QuestionCard.tsx";
import Requests from "../components/conversation/questions/Requests/Requests.tsx";
import {
  RequestFixture,
  createRequestFixture,
} from "../components/conversation/questions/support/requestFixtures.ts";
import { agentSummaryFixture } from "../servers/agentSummaryFixture.ts";
import { expect } from "storybook/test";
import {
  NativeError,
  NativeNotice,
} from "../components/shell/NativeNotice/NativeNotice.tsx";
import type { Agent, Message } from "../types.ts";
import { computedContrast } from "./computedContrast.ts";
import "./foundation.css";

function NoticeAudit() {
  return (
    <main className="foundation-sheet">
      <h1>Operational notices</h1>
      {["var(--studio-page)", "var(--surface)", "var(--raised)"].map(
        (background) => (
          <section
            key={background}
            style={{ background, padding: "var(--mantine-spacing-sm)" }}
          >
            <NativeError
              agent={
                {
                  id: "notice-agent",
                  kind: "agent",
                  name: "Release lead",
                  source: AgentSource.managed,
                  status: AgentStatus.failed,
                  created: 1,
                  error:
                    "The provider connection failed. Your input is retained.",
                } satisfies Agent
              }
            />
            {(["error", "warning"] as const).map((kind) => (
              <NativeNotice
                key={kind}
                item={
                  {
                    id: kind,
                    role: "assistant",
                    text:
                      kind === "error"
                        ? "The provider connection failed."
                        : "Review the provider warning.",
                    nativeNotice: kind,
                  } as Message
                }
              />
            ))}
            <div className="native-error warning">
              <span>Wait for model capacity.</span>
            </div>
            <div className="native-error info">
              <span>The connection recovered.</span>
            </div>
            <div className="native-notice info">
              <details>
                <summary>Review the connection result.</summary>
              </details>
            </div>
            <div className="native-policy-composer-warning">
              This request needs a policy review.
            </div>
            <div className="native-limit-notice-heading">
              <strong>Account allowance reached</strong>
            </div>
          </section>
        ),
      )}
    </main>
  );
}
const meta = {
  title: "Design/Semantic audit",
  component: NoticeAudit,
  beforeEach: setupRequestAudit,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof NoticeAudit>;
export default meta;
type Story = StoryObj<typeof meta>;
async function verifyNotices(canvasElement: HTMLElement) {
  const labels = canvasElement.querySelectorAll(
    ".native-error > span, .native-notice summary, .native-policy-composer-warning, .native-limit-notice-heading strong",
  );
  expect(labels.length).toBeGreaterThan(12);
  for (const label of labels) {
    const result = computedContrast(label);
    expect(
      result.ratio,
      `${label.textContent}: ${JSON.stringify(result)}`,
    ).toBeGreaterThanOrEqual(4.5);
  }
}
export const NoticesLight: Story = {
  globals: { colorScheme: "light" },
  play: ({ canvasElement }) => verifyNotices(canvasElement),
};
export const NoticesDark: Story = {
  globals: { colorScheme: "dark" },
  play: ({ canvasElement }) => verifyNotices(canvasElement),
};

let auditOwner: ReturnType<typeof createRequestFixture>;
function setupRequestAudit() {
  const requests = [
    approval({
      id: "audit-question",
      agent: "audit-lead",
      request: {
        kind: "agent_question",
        questions: [
          question("scope", "Which files should I review?", [
            "One file",
            "All files",
          ]),
        ],
      },
    }),
  ];
  const fixture = createRequestFixture({
    scope: "semantic-audit",
    notify() {},
    requests: [
      ...requests,
      approval({
        id: "audit-signin",
        agent: "audit-lead",
        request: browserOpen("https://example.com/device", {
          host: "example.com",
          userCode: "ABCD-1234",
        }),
      }),
    ],
    agents: [agentSummaryFixture("audit-lead", { name: "Release lead" })],
    details: [{ id: "audit-lead" }],
  });
  fixture.start();
  auditOwner = fixture;
  return () => fixture.dispose();
}
function PrimaryAudit() {
  return (
    <main className="foundation-sheet">
      <span
        data-audit-primary
        style={{ background: "var(--mantine-primary-color-filled)" }}
      >
        Primary color
      </span>
      <QuestionCard
        card={{
          source: "message",
          state: "waiting",
          asynchronous: true,
          requestId: "audit-question",
          questions: [
            {
              id: "scope",
              question: "Which scope?",
              options: [],
              multiSelect: false,
              isSecret: false,
            },
          ],
        }}
        onAnswer={() => {}}
      />
      <RequestFixture owner={auditOwner}>
        <Requests all />
      </RequestFixture>
      <Button variant="filled">Create project</Button>
      <Button data-audit-quiet>Cancel</Button>
    </main>
  );
}
async function verifyPrimary(canvasElement: HTMLElement) {
  const expected = getComputedStyle(
    canvasElement.querySelector("[data-audit-primary]")!,
  ).backgroundColor;
  const actions = canvasElement.querySelectorAll(
    '[data-variant="filled"]:not(:disabled)',
  );
  expect(actions.length).toBeGreaterThanOrEqual(4);
  for (const action of actions) {
    expect(
      getComputedStyle(action).backgroundColor,
      action.textContent || "Primary action",
    ).toBe(expected);
    expect(
      computedContrast(action).ratio,
      action.textContent || "Primary action",
    ).toBeGreaterThanOrEqual(4.5);
  }
  expect(
    getComputedStyle(canvasElement.querySelector("[data-audit-quiet]")!).color,
  ).not.toBe(expected);
}
export const PrimaryLight = {
  render: () => <PrimaryAudit />,
  globals: { colorScheme: "light" },
  play: ({ canvasElement }) => verifyPrimary(canvasElement),
} satisfies Story;
export const PrimaryDark: Story = {
  render: () => <PrimaryAudit />,
  globals: { colorScheme: "dark" },
  play: ({ canvasElement }) => verifyPrimary(canvasElement),
};

function RaisedAudit() {
  return (
    <main className="foundation-sheet" style={{ background: "var(--raised)" }}>
      <span className="team-button-count">12</span>
      <span className="attention-count">3</span>
    </main>
  );
}
async function verifyRaised(canvasElement: HTMLElement) {
  for (const label of canvasElement.querySelectorAll(
    ".team-button-count,.attention-count",
  )) {
    const result = computedContrast(label);
    expect(
      result.ratio,
      `${label.className}: ${JSON.stringify(result)}`,
    ).toBeGreaterThanOrEqual(4.5);
  }
}
export const RaisedLight: Story = {
  render: () => <RaisedAudit />,
  globals: { colorScheme: "light" },
  play: ({ canvasElement }) => verifyRaised(canvasElement),
};
export const RaisedDark: Story = {
  render: () => <RaisedAudit />,
  globals: { colorScheme: "dark" },
  play: ({ canvasElement }) => verifyRaised(canvasElement),
};

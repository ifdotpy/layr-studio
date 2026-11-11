import {
  observableFixture,
  conversationActionsFixture,
} from "../../shell/integration-support/observableFixture.tsx";
import { ConversationProvider } from "./navigation.ts";
import { ChatIndicatorKind as ChatOperationalKind } from "../../../status/operationalStatus.ts";
import {
  AgentSource as GeneratedApiAgentSource,
  AgentStatus as GeneratedApiAgentStatus,
  ProviderKind as GeneratedApiProviderKind,
} from "../../../generated/api.ts";
import type { WorkerModelInfo } from "../../agents/support/WorkerModelPicker.tsx";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import { ActionIcon, Button, Textarea } from "@mantine/core";
import {
  ArrowUp,
  ListEnd,
  Mail,
  Settings2,
  PanelLeft,
  Users,
} from "lucide-react";
import ConversationTitle from "../../shell/ConversationTitle/ConversationTitle.tsx";
import ConversationStart from "../LiveConversationStart/LiveConversationStart.tsx";
import { ExecutionSettingsFixture as ExecutionSettings } from "../../shell/integration-support/observableFixture.tsx";
import type { UsageAccount } from "../../../app/settings/UsageFixture.tsx";
import {
  UsageFixture as Usage,
  createUsageFixture,
} from "../../../app/settings/UsageFixture.tsx";
import { createServerServices } from "../../../servers/serverServices.ts";
import { ServerServicesProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { Agent } from "../../../types.ts";
import type { AccountLimitsSnapshot } from "../../../usage/accountUsage.ts";
import type { ChatIndicator } from "../../../status/operationalStatus.ts";
import "./empty-chat-settings.css";

const indicators: ChatIndicator[] = [
  { kind: ChatOperationalKind["answer"], label: "Needs your answer" },
  { kind: ChatOperationalKind["error"], label: "Failed" },
  { kind: ChatOperationalKind["working"], label: "Working" },
  { kind: ChatOperationalKind["unread"], label: "Unread result" },
  { kind: ChatOperationalKind["paused"], label: "Stopped" },
  { kind: ChatOperationalKind["none"], label: "Ready" },
];
const titles = [
  "Обновление обработчика запросов и восстановление сессии после потери соединения",
  "Repair the deployment pipeline and keep the previous release recoverable",
  "Review permissions for parallel project workspaces",
  "Extract the shared account usage display",
  "Проверка интерфейса нескольких проектов на узком экране",
  "Prepare the next release",
] as const;
const recentChats = indicators.map((indicator, index) => {
  const title = titles[index];
  if (title === undefined)
    throw new Error("Missing conversation title fixture");
  return { id: `chat-${index}`, title, indicator };
});
const lead = {
  id: "story-lead",
  rootId: "story-lead",
  name: "Release coordination",
  source: GeneratedApiAgentSource.managed,
  isLead: true,
  status: GeneratedApiAgentStatus.idle,
  model: "gpt-6-astra",
  effort: "high",
  provider: GeneratedApiProviderKind.codex,
  cwd: "/projects/codex-studio",
  accountKey: "default",
  workerModel: "gpt-6-luna",
  contextUsage: { tokens: 64000, window: 200000 },
} as Agent;
const models = ["gpt-6-astra", "gpt-6-luna"].map((model): WorkerModelInfo => ({
  model,
  displayName: model,
  provider: GeneratedApiProviderKind.codex,
  defaultReasoningEffort: "high",
  serviceTiers: [],
  supportedReasoningEfforts: [{ reasoningEffort: "high" }],
}));
const catalog = { models, loading: false, error: "", retry: fn() };
const limits: AccountLimitsSnapshot = {
  accountKey: "default",
  data: {
    accountId: "story-account",
    rateLimits: {
      planType: "team",
      primary: {
        usedPercent: 28,
        windowDurationMins: 300,
        resetsAt: 2000000000,
      },
      secondary: {
        usedPercent: 43,
        windowDurationMins: 10080,
        resetsAt: 2000000000,
      },
    },
  },
};
const observedAt = Date.now() / 1000;
const usageAccounts = [10, 15, 40].map((usedPercent, index): UsageAccount => {
  const key = index === 0 ? "default" : `account-${index}`;
  const label = ["Engineering", "Research", "Review"][index];
  if (label === undefined) throw new Error("Missing account label fixture");
  return {
    key,
    label,
    provider: index === 2 ? "claude" : "codex",
    loading: false,
    reload: fn(),
    limits: {
      accountKey: key,
      at: observedAt,
      data: {
        rateLimits: {
          primary: {
            usedPercent: 43,
            windowDurationMins: 300,
            resetsAt: observedAt + 3600,
          },
          secondary: {
            usedPercent,
            windowDurationMins: 10080,
            resetsAt: observedAt + 6 * 86400,
          },
        },
      },
    },
  };
});
const sessionCost = {
  rootId: lead.id,
  pricingState: "ready",
  estimated: true,
  totalUSD: 4.28,
  unknownModels: [],
  breakdown: { providers: { codex: 4.28 } },
};
const services = createServerServices({
  id: "local",
  label: "Story fixture",
  origin: window.location.origin,
});
services.api.get = (async (path: string) => {
  if (path === "/api/session-cost") return sessionCost;
  if (path === "/api/models") return { data: models };
  throw new Error(`Unexpected story request: ${path}`);
}) as typeof services.api.get;
services.watchResourceReads = (_resources, read, failed) => {
  void read().catch(failed);
  return Object.assign(() => {}, {
    refresh: () => {
      void read().catch(failed);
    },
  });
};

const projectNames = ["codex-studio", "af-utils", "attar"];
const reasonByKind = {
  working: "working",
  answer: "needs_input",
  error: "failed",
  unread: "unread_result",
  paused: "stopped",
  none: "ready",
  loading: "ready",
} as const;
const titleFixture = observableFixture(
  projectNames.flatMap((projectName, projectIndex) => [
    {
      ...lead,
      id: projectIndex === 0 ? lead.id : projectName,
      name: titles[0],
      isLead: true,
      projectPath: `/projects/${projectName}`,
    },
    ...recentChats.map((chat, index): Agent => ({
      id: projectIndex === 0 ? chat.id : `${projectName}-${chat.id}`,
      source: GeneratedApiAgentSource["managed"],
      isLead: true,
      name: chat.title,
      projectPath: `/projects/${projectName}`,
      created: index,
      operationalStatus:
        chat.indicator?.kind === "answer"
          ? { reason: "needs_input", wait: null }
          : { reason: reasonByKind[chat.indicator?.kind || "none"] },
    })),
  ]),
);
titleFixture.apply([
  ...[...titleFixture.mirror.current.agents.values()].map((value) => ({
    collection: "agentSummary" as const,
    id: value.id,
    value,
  })),
  ...projectNames.map((name) => ({
    collection: "project" as const,
    id: name,
    value: { id: name, name, path: `/projects/${name}` },
  })),
]);
function StartFixture({
  id = lead.id,
  open,
}: {
  id?: string;
  open: (id: string) => void;
}) {
  return (
    <titleFixture.Provider>
      <ConversationProvider value={conversationActionsFixture(open)}>
        <ConversationStart agentId={id} />
      </ConversationProvider>
    </titleFixture.Provider>
  );
}
function TitleFixture() {
  return (
    <titleFixture.Provider>
      <ConversationProvider value={conversationActionsFixture(fn())}>
        <ConversationTitle
          agent={titleFixture.mirror.current.agents.get(lead.id)}
          serverAlias="MAC"
        />
      </ConversationProvider>
    </titleFixture.Provider>
  );
}

let usageFixture: ReturnType<typeof createUsageFixture>;
const meta = {
  title: "Conversation/Dispatch chrome",
  beforeEach: () => {
    usageFixture = createUsageFixture(titleFixture.store, services.api, {
      agent: lead,
      accounts: usageAccounts,
    });
    usageFixture.start();
    return () => usageFixture.dispose();
  },
  parameters: { layout: "fullscreen" },
  args: { onSelect: fn(), onTeam: fn() },
  render: (args: { onSelect: (id: string) => void; onTeam: () => void }) => (
    <ServerServicesProvider services={services}>
      <titleFixture.Provider>
        <main className="workspace" style={{ position: "fixed", inset: 0 }}>
          <header className="workspace-header simple-workspace-header">
            <ActionIcon aria-label="Toggle conversations">
              <PanelLeft size={18} />
            </ActionIcon>
            <TitleFixture />
            <Button
              aria-label="Team"
              title="Team: 1 working of 6"
              leftSection={<Users size={16} />}
              onClick={args.onTeam}
            >
              Team
            </Button>
            <Button
              variant="subtle"
              leftSection={<Mail size={16} />}
              aria-label="Messages"
            >
              <span className="messages-toggle-label">Messages</span>
            </Button>
            <ActionIcon aria-label="Chat actions">
              <Settings2 size={18} />
            </ActionIcon>
          </header>
          <section id="conversation">
            <div id="messages">
              <div className="message-content">
                <StartFixture open={args.onSelect} />
              </div>
            </div>
            <form id="composer">
              <Textarea
                aria-label="Message"
                placeholder="Describe the work…"
                autosize
                minRows={1}
              />
              <div className="composer-bar">
                <div className="composer-tools">
                  <div
                    className="empty-chat-settings composer-context-row"
                    aria-label="Agent settings"
                  >
                    {(["orchestrator", "worker"] as const).map((role) => (
                      <div className="empty-chat-setting" key={role}>
                        <ExecutionSettings
                          agent={lead}
                          catalog={catalog}
                          settingsRow
                          initialRole={role}
                        />
                      </div>
                    ))}
                  </div>
                </div>
                <div className="composer-submit-actions">
                  <ActionIcon aria-label="Queue for next turn">
                    <ListEnd size={18} />
                  </ActionIcon>
                  <ActionIcon aria-label="Send message">
                    <ArrowUp size={18} />
                  </ActionIcon>
                </div>
              </div>
            </form>
            <div className="conversation-composer-footer">
              <Usage
                owner={usageFixture}
                agent={lead}
                stateDir="story-dispatch"
                limits={limits}
                accounts={usageAccounts}
                accountLabel="Engineering"
                reload={fn()}
              />
            </div>
          </section>
        </main>
      </titleFixture.Provider>
    </ServerServicesProvider>
  ),
} satisfies Meta<{ onSelect: (id: string) => void; onTeam: () => void }>;
export default meta;
type Story = StoryObj<typeof meta>;
export const ProjectStart: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("heading", { name: "codex-studio" }),
    ).toBeVisible();
    await expect(canvas.getByText("/projects/codex-studio")).toBeVisible();
    await expect(
      canvas.getByRole("button", { name: "Chat context" }),
    ).toHaveTextContent("Context 32%");
    await expect(
      canvas.getByRole("button", { name: "Account limits" }),
    ).toHaveTextContent("Limits");
    for (const color of ["green", "yellow", "red"]) {
      await expect(
        canvasElement.querySelector(
          `.account-limits-dot[data-color="${color}"]`,
        ),
      ).toBeVisible();
    }
    await userEvent.click(canvas.getByRole("button", { name: "Team" }));
    await expect(args.onTeam).toHaveBeenCalledOnce();
    await userEvent.click(
      within(
        canvas.getByRole("navigation", { name: "Recent project chats" }),
      ).getByRole("button", { name: /Prepare the next release/ }),
    );
    await expect(args.onSelect).toHaveBeenCalledWith("chat-5");
  },
};
export const SeveralProjects: Story = {
  render: (args) => (
    <div>
      {["codex-studio", "af-utils", "attar"].map((projectName) => (
        <StartFixture
          key={projectName}
          id={projectName === "codex-studio" ? lead.id : projectName}
          open={args.onSelect}
        />
      ))}
    </div>
  ),
};

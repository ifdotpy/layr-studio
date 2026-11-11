import { browserAreas } from "../fixtures/owner-support.tsx";
import { ServerAreasContextProvider as AreasProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import "../../src/rendererStyles.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { Agent } from "../../src/types.ts";
import type { components } from "../../src/generated/api.ts";
const rootElement = document.getElementById("root")!;
import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider, ActionIcon, Textarea } from "@mantine/core";
import { Paperclip, Mic, Square, ArrowUp, ListEnd } from "lucide-react";
import "@mantine/core/styles.css";
import "../../src/style.css";
import "../../src/appearance.css";
import "../../src/components/conversation/support/chat-controls.css";
import "../../src/components/conversation/support/conversation-chrome.css";
import "../../src/components/conversation/support/empty-chat-settings.css";
import { theme } from "../../src/theme.ts";
import { ExecutionSummaryTrigger } from "../../src/components/agents/ExecutionSummaryTrigger/ExecutionSummaryTrigger.tsx";
import { ExecutionSettingsFixture as ExecutionSettings } from "../../src/components/shell/integration-support/observableFixture.tsx";
import {
  ModelCatalogFixture,
  createModelCatalogFixture,
  type ModelCatalogFixtureSource,
} from "../../src/components/servers/ServerServicesProvider/modelCatalogFixture.tsx";
const query = new URLSearchParams(location.search);
const name =
  query.get("long") === "word"
    ? "ExtremelyLongUnbrokenModelIdentifierForTheComposerRegression"
    : query.has("long")
      ? "Opus extended context model with a long descriptive label"
      : "Opus 5.5";
const models = [
  {
    model: "opus",
    displayName: name,
    provider: "claude",
    supportedReasoningEfforts: [],
    serviceTiers: [],
  },
  {
    model: "sol",
    displayName: "Sol 6.1",
    provider: "codex",
    supportedReasoningEfforts: [],
    serviceTiers: [],
  },
  {
    // 9c5ebefc2 only offers explicit GPT models for review.
    model: "gpt-6.1-sol",
    displayName: "Sol 6.1 review",
    provider: "codex",
    supportedReasoningEfforts: [],
    serviceTiers: [],
  },
] satisfies components["schemas"]["ProviderModel"][];
const reader: ModelCatalogFixtureSource = {
  scope: "composer-model-summary",
  read: async () => ({ data: models }),
  subscribeState: (accept) => {
    accept({
      runtime: {
        modelsByAccount: {
          default: models.map((row, catalogPosition) => ({
            ...row,
            accountKey: "default",
            catalogPosition,
          })),
        },
      },
      entitiesComplete: true,
    });
    return () => {};
  },
};
const catalog = createModelCatalogFixture(reader);
window.addEventListener("pagehide", catalog.dispose, { once: true });
type ComposerLead = Omit<Agent, "workerDefaults"> & {
  workerDefaults: Omit<NonNullable<Agent["workerDefaults"]>, "fastMode">;
};
// The base one-off composer lead omits workerDefaults.fastMode.
function partialComposerLead(value: ComposerLead): Agent {
  return value as Agent;
}
const lead = partialComposerLead({
  id: "lead",
  isLead: true,
  source: "managed",
  accountKey: "default",
  provider: "claude",
  model: "opus",
  effort: "high",
  status: query.has("active") ? "running" : "idle",
  workerDefaults: { model: "sol", effort: "high" },
  reviewDefaults: { model: null, effort: null },
});
const agent = query.has("worker")
  ? { ...lead, id: "worker", isLead: false, rootId: "lead" }
  : lead;
const modelsOwner = observableFixture(
  agent.id === lead.id ? [lead] : [lead, agent],
);
modelsOwner.store.models = () => ({
  get current() {
    return { status: "live" as const, data: catalog.models.current };
  },
});
const areas = browserAreas(modelsOwner.store);
window.addEventListener("pagehide", modelsOwner.store.dispose, { once: true });
function Fixture() {
  return (
    <MantineProvider
      theme={theme}
      forceColorScheme={(query.get("theme") || "dark") as "light" | "dark"}
    >
      <modelsOwner.Provider>
        <ModelCatalogFixture value={catalog}>
          <AreasProvider value={areas}>
            <main
              style={{
                padding: 8,
                width: "100%",
                maxWidth: 1200,
                margin: "auto",
                alignSelf: "end",
              }}
            >
              <form id="composer" onSubmit={(e) => e.preventDefault()}>
                <Textarea
                  id="message"
                  aria-label="Message"
                  placeholder="What should we work on?"
                />
                <div className="composer-bar">
                  <div className="composer-tools">
                    <ActionIcon aria-label="Attach files">
                      <Paperclip />
                    </ActionIcon>
                    <div
                      className="empty-chat-settings composer-context-row"
                      aria-label="Agent settings"
                    >
                      <ExecutionSettings
                        agent={agent}
                        lead={lead}
                        catalog={{
                          models,
                          loading: false,
                          error: "",
                          retry: () => {},
                        }}

                        renderTrigger={({ agent, opened, ref, onClick }) => (
                          <ExecutionSummaryTrigger
                            agent={agent}
                            lead={lead}
                            team={[lead, agent]}
                            ref={ref}
                            aria-expanded={opened}
                            onOpen={onClick}
                          />
                        )}
                      />
                    </div>
                  </div>
                  <div className="composer-submit-actions">
                    <ActionIcon aria-label="Dictate">
                      <Mic />
                    </ActionIcon>
                    <ActionIcon
                      className="composer-queue-primary"
                      aria-label="Queue for next turn"
                    >
                      <ListEnd />
                    </ActionIcon>
                    {query.has("active") && (
                      <ActionIcon id="stop" aria-label="Stop">
                        <Square />
                      </ActionIcon>
                    )}
                    <ActionIcon id="send" aria-label="Send message">
                      <ArrowUp />
                    </ActionIcon>
                  </div>
                </div>
              </form>
            </main>
          </AreasProvider>
        </ModelCatalogFixture>
      </modelsOwner.Provider>
    </MantineProvider>
  );
}
createRoot(rootElement).render(<Fixture />);

import "../../src/rendererStyles.ts";
import {
  PublishedAgentFixture,
  usePublishedAgent,
  publishFixtureAgent,
} from "../fixtures/PublishedAgentFixture.tsx";
import type { Agent } from "../../src/types.ts";
import type { GetOptions } from "../../src/api.ts";
import type { PostBody, PostResult } from "../../src/apiTypes.ts";
import {
  fixtureFetchInput,
  fixturePostBody,
  fixtureAccount,
} from "./fixture-helpers.ts";
export type CanonicalReset = {
  agent?: Partial<
    Pick<
      Agent,
      | "id"
      | "accountKey"
      | "model"
      | "effort"
      | "fastMode"
      | "daybreakEnabled"
      | "status"
      | "reviewDefaults"
      | "accountTransfer"
      | "pendingSettings"
      | "pendingSettingsAccountKey"
      | "isLead"
      | "provider"
      | "permissionMode"
    >
  > & { workerDefaults?: Partial<NonNullable<Agent["workerDefaults"]>> | null };
  defaults?: boolean;
};

declare global {
  interface Window {
    catalogRequests: string[];
    // Renamed from calls: chat/chat-read-client-browser.fixture.tsx declares read-state writes.
    settingsCalls: PostBody<"/api/conversation">[];
    transfers: PostBody<"/api/agents/account-transfer">[];
    transferFail: boolean;
    transferHold: boolean;
    transferReply: Partial<PostResult<"/api/agents/account-transfer">> | null;
    pendingTransferProjection: Agent | null;
    reply: Agent | null;
    // Renamed from fail: chat/chat-read-client-browser.fixture.tsx declares a boolean.
    canonicalFail: string | null;
    hold: boolean;
    projectionUnavailable: boolean;
    projectionHold: boolean;
    refreshCount: number;
    catalogHold: boolean;
    catalogRelease: (() => void) | null;
    // Renamed from release: chat/chat-read-client-browser.fixture.tsx declares a non-null callback.
    canonicalRelease: (() => void) | null;
    // Renamed from refreshRelease: teams/subagent-concurrency-confirmed-browser.fixture.tsx allows undefined.
    canonicalProjectionRelease: (() => void) | null;
    transferRelease: (() => void) | null;
    serverAccount: string | null;
    agent: Agent;
    setAgent: (value: Agent) => void;
    // Renamed from reset: settings/daybreak-settings-browser.fixture.tsx takes DaybreakReset options.
    canonicalReset: (options?: CanonicalReset) => Promise<void>;
  }
}
const fixtureRoot = document.getElementById("root");
if (!fixtureRoot) throw new Error("Missing fixture root");

import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
} from "../../src/generated/apiSchema.ts";
const fixtureSchemaHeaders = {
  "Content-Type": "application/json",
  [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
};
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import {
  ExecutionSettings,
  ExecutionPermissions,
} from "../../src/components/agents/ExecutionSettings/ExecutionSettings.tsx";
import { observer } from "mobx-react-lite";
import { useServerStore } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../src/store/runtime.ts";
import { agentId } from "../../src/generated/observableResources.ts";
import type { ComponentProps } from "react";
const CanonicalSettings = observer(function CanonicalSettings({
  agent,
  permissions,
  ...props
}: Omit<ComponentProps<typeof ExecutionSettings>, "input"> & {
  agent: Agent;
  permissions?: boolean;
}) {
  const store = useServerStore();
  const summary = resourceData(store.state().current)?.agents.get(agent.id);
  const detail = resourceData(store.agentDetail(agentId(agent.id)).current);
  if (!summary) return null;
  const Component = permissions ? ExecutionPermissions : ExecutionSettings;
  return <Component {...props} input={{ kind: "agent", summary, detail }} />;
});
import {
  ModelCatalogFixture,
  createModelCatalogFixture,
  type ModelCatalogFixtureSource,
} from "../../src/components/servers/ServerServicesProvider/modelCatalogFixture.tsx";
import { get } from "../../src/servers/net/api.ts";
const fixtureModelRead = (
  options: Pick<GetOptions<"/api/models">, "query" | "signal">,
) => get("/api/models", options);
import { browserAreas } from "../fixtures/owner-support.tsx";
import { fixtureServerStore } from "../fixtures/PublishedAgentFixture.tsx";
const areas = browserAreas(fixtureServerStore);
const initial: Agent = {
  id: "first",
  accountKey: "default",
  name: "First",
  source: "managed",
  model: "gpt-6-astra",
  effort: "low",
  fastMode: false,
  isLead: true,
  status: "idle",
  created: 1,
  permissionMode: "default",
  nextTurnSettingsSupported: true,
};
// Partial fixture view: the base fixture omits inherited worker model, effort and fastMode.
function partialAgent(
  value: Omit<Agent, "workerDefaults"> & {
    workerDefaults?:
      | Partial<NonNullable<Agent["workerDefaults"]>>
      | Agent["workerDefaults"];
  },
): Agent {
  return value as Agent;
}
const models = ["gpt-6-astra", "gpt-5.6-sol", "gpt-5.6-luna", "gpt-6-luna"].map(
  (model) => ({
    model,
    supportedReasoningEfforts: ["low", "medium", "high", "max"].map(
      (reasoningEffort) => ({ reasoningEffort }),
    ),
    serviceTiers: [{ id: "priority" }],
    availableAccessPrograms: { cyber: ["standard", "daybreakBlue"] },
  }),
);
window.catalogRequests = [];
window.settingsCalls = [];
window.transfers = [];
window.transferFail = false;
window.transferHold = false;
window.transferReply = null;
window.pendingTransferProjection = null;
window.reply = null;
window.canonicalFail = null;
window.hold = false;
window.projectionUnavailable = false;
window.projectionHold = false;
window.refreshCount = 0;

const fixtureResponse = (value: unknown, init: ResponseInit = {}) =>
  new Response(JSON.stringify(value), {
    ...init,
    headers: {
      ...fixtureSchemaHeaders,
      ...init.headers,
    },
  });
function publishSettings(value: Agent | null) {
  const publish = () => {
    if (value && !window.projectionUnavailable) window.setAgent(value);
  };
  if (window.projectionHold) window.canonicalProjectionRelease = publish;
  else queueMicrotask(publish);
}
const nativeFetch = window.fetch;
window.fetch = async (input, options) => {
  const { url, options: requestOptions } = await fixtureFetchInput(
    input,
    options,
    true,
  );
  if (/^\/api\/(?:session|sync\/pull)(?:\?|$)/.test(String(url)))
    window.refreshCount++;
  if (String(url).startsWith("/api/models?")) {
    window.catalogRequests.push(url);
    if (window.catalogHold)
      await new Promise<void>((resolve) => (window.catalogRelease = resolve));
    return fixtureResponse({
      data: String(url).includes("account_key=claude")
        ? [
            {
              model: "claude-opus-4-6",
              provider: "claude",
              displayName: "Claude Opus 4.6",
              isDefault: true,
              supportedReasoningEfforts: [{ reasoningEffort: "high" }],
            },
          ]
        : models,
    });
  }
  if (url == "/api/agents/account-transfer") {
    const body = fixturePostBody<"/api/agents/account-transfer">(
      requestOptions?.body,
    );
    window.transfers.push(body);
    if (window.transferHold)
      await new Promise<void>((resolve) => (window.transferRelease = resolve));
    if (window.transferFail) throw new TypeError("Connection lost");
    const result = window.transferReply || {
      id: body.request_id,
      leadId: "first",
      scope: body.scope,
      status: "completed",
      targetAccountKey: body.account_key,
      completed: 2,
      moved: 2,
      total: 3,
      leftOnSource: [
        {
          id: "w-claude",
          name: "Claude worker",
          provider: "claude",
          reason: "Uses claude",
        },
      ],
    };
    window.pendingTransferProjection = partialAgent({
      ...window.agent,
    });
    publishSettings(window.pendingTransferProjection);
    return fixtureResponse(result);
  }
  if (url != "/api/conversation") return nativeFetch(url, requestOptions);
  window.settingsCalls.push(
    fixturePostBody<"/api/conversation">(requestOptions?.body),
  );
  if (
    window.serverAccount &&
    window.settingsCalls.at(-1)!.expected_account_key !== window.serverAccount
  )
    return fixtureResponse(
      { error: "The account changed. Select the model again" },
      { status: 409 },
    );
  if (window.hold)
    await new Promise<void>((resolve) => (window.canonicalRelease = resolve));
  if (window.canonicalFail === "network")
    throw new TypeError("Connection lost");
  if (!window.canonicalFail) publishSettings(window.reply);
  return fixtureResponse(
    window.canonicalFail ? { error: "Rejected" } : window.reply,
    {
      status: window.canonicalFail ? 409 : 200,
    },
  );
};
const team: Agent[] = [
  {
    id: "w1",
    name: "Codex worker",
    rootId: "first",
    provider: "codex",
    accountKey: "default",
  },
  {
    id: "w2",
    name: "Second codex worker",
    rootId: "first",
    provider: "codex",
    accountKey: "default",
  },
  {
    id: "w3",
    name: "Claude worker",
    rootId: "first",
    provider: "claude",
    accountKey: "claude",
  },
] satisfies Agent[];
let catalogMount = 0;
function createCatalog() {
  const catalogSource: ModelCatalogFixtureSource = {
    scope: "settings-canonical-" + catalogMount,
    read: fixtureModelRead,
    subscribeState: (accept) => {
      accept({
        entitiesComplete: true,
        runtime: {
          modelsByAccount: Object.fromEntries(
            ["default", "second"].map((accountKey) => [
              accountKey,
              models.map((model, catalogPosition) => ({
                ...model,
                accountKey,
                catalogPosition,
                provider: "codex" as const,
              })),
            ]),
          ),
        },
      });
      return () => {};
    },
  };
  return createModelCatalogFixture(catalogSource);
}
let catalog = createCatalog();
window.addEventListener("pagehide", () => catalog.dispose(), { once: true });
const Fixture = observer(function Fixture() {
  const [selectedId, setSelectedId] = useState(initial.id),
    [mount, setMount] = useState(0),
    [defaults, setDefaults] = useState(false);
  const agent = usePublishedAgent(selectedId, initial);
  window.setAgent = async (value) => {
    setSelectedId(value.id);
    await publishFixtureAgent(value);
  };
  window.agent = agent;
  window.canonicalReset = async (options) => {
    await window.setAgent(partialAgent({ ...initial, ...options?.agent }));
    flushSync(() => {
      setDefaults(!!options?.defaults);
      catalog.dispose();
      catalogMount++;
      catalog = createCatalog();
      setMount((value) => value + 1);
      window.catalogRequests = [];
      window.settingsCalls = [];
      window.transfers = [];
      window.transferFail = false;
      window.transferHold = false;
      window.transferReply = null;
      window.pendingTransferProjection = null;
      window.catalogHold = false;
      window.reply = null;
      window.canonicalFail = null;
      window.hold = false;
      window.projectionUnavailable = false;
      window.projectionHold = false;
      window.refreshCount = 0;
      window.canonicalRelease = null;
      window.canonicalProjectionRelease = null;
      window.transferRelease = null;
      window.serverAccount = null;
    });
  };

  return (
    <MantineProvider>
      <ModelCatalogFixture value={catalog}>
        <CanonicalSettings
          agentsUi={areas.agentsUi}
          key={mount}
          settingsRow={defaults}
          initialRole={defaults ? "worker" : "orchestrator"}
          agent={agent}
          accounts={[
            fixtureAccount({
              id: "default",
              label: "",
              email: "first@example.com",
              status: "ready",
            }),
            fixtureAccount({
              id: "second",
              label: "",
              email: "second@example.com",
              status: "ready",
            }),
            fixtureAccount({
              id: "claude",
              label: "Claude",
              provider: "claude",
              status: "ready",
            }),
            fixtureAccount({
              id: "offline",
              label: "Offline",
              status: "error",
            }),
          ]}
          team={team}
          catalog={{ models, loading: false, error: "", retry: () => {} }}
        />
        {!defaults && (
          <CanonicalSettings
            agentsUi={areas.agentsUi}
            permissions
            key={"permissions:" + mount}
            agent={agent}
            catalog={{ models, loading: false, error: "", retry: () => {} }}
          />
        )}
      </ModelCatalogFixture>
    </MantineProvider>
  );
});
await publishFixtureAgent(initial);
createRoot(fixtureRoot!).render(
  <PublishedAgentFixture>
    <Fixture />
  </PublishedAgentFixture>,
);

// @vitest-environment happy-dom
import { reaction } from "mobx";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../shell/integration-support/observableFixture.tsx";
vi.mock("mobx-react-lite", async (original) => ({
  ...(await original<typeof import("mobx-react-lite")>()),
  observer: <T>(component: T) => component,
}));
vi.mock("../support/state.ts", async (original) => ({
  ...(await original<typeof import("../support/state.ts")>()),
  useAgentDetail: <T>(agent: T) => agent,
}));
import {
  AccountStatus,
  AgentSource,
  AgentStatus,
  ProviderKind,
} from "../../../generated/api.ts";
import { createElement } from "react";
import { renderToStaticMarkup as renderMarkup } from "react-dom/server";
import { ServerAreasContextProvider as AreasProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
const renderToStaticMarkup = (node: Parameters<typeof renderMarkup>[0]) =>
  renderMarkup(createElement(AreasProvider, { value: areas }, node));
import { MantineProvider } from "@mantine/core";
import { beforeEach, expect, test, vi } from "vitest";
import { ExecutionSettings } from "./ExecutionSettings.tsx";
import { ExecutionSummaryTrigger } from "../ExecutionSummaryTrigger/ExecutionSummaryTrigger.tsx";
import { INHERITED_MODEL } from "../../settings/models/support/modelOption.ts";
import type { AgentSetupPickerProps } from "../AgentSetupPicker/AgentSetupPicker.tsx";
import type { Agent } from "../../../types.ts";

let areas: ReturnType<typeof conversationActionsFixture>;
let stopFields: (() => void) | undefined;
const generatedFixtures: ReturnType<typeof observableFixture>[] = [];
function settingsInput(agent: Agent) {
  const f = observableFixture([agent]);
  generatedFixtures.push(f);
  return {
    kind: "agent" as const,
    summary: f.mirror.current.agents.get(agent.id)!,
    detail: f.details.get(agent.id)!.current,
  };
}
const choices = vi.hoisted(() => ({
  pickers: [] as AgentSetupPickerProps[],
  states: [] as unknown[],
  terminal: undefined as (() => void) | undefined,
  read: vi.fn(),
  post: vi.fn(),
}));

const capability = vi.hoisted(() => ({ priority: false }));

test("the default non-lead summary retains inherited worker and review roles", () => {
  const lead = settingsInput({ id: "lead", isLead: true });
  const worker = settingsInput({
    id: "worker",
    rootId: "lead",
    model: "gpt-6-luna",
  });
  const markup = renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(ExecutionSettings, {
        agentsUi: areas.agentsUi,
        input: worker,
        leadInput: lead,
        team: [lead.summary, worker.summary],
        catalog: { models: [], loading: false, error: "", retry: vi.fn() },
      }),
    ),
  );
  expect(markup).toContain('data-role="worker"');
  expect(markup).toContain('data-role="review"');
});

test.each([null, INHERITED_MODEL, "gpt-6-luna"])(
  "review settings omit caller-model choices with stored model %s",
  (model) => {
    const agent: Agent = {
      id: "review-picker",
      isLead: true,
      source: AgentSource.managed,
      status: AgentStatus.idle,
      provider: ProviderKind.codex,
      model: "gpt-6.1-sol",
      reviewDefaults: { model, effort: null },
    };
    const markup = renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(ExecutionSettings, {
          agentsUi: areas.agentsUi,
          presentation: "panel",
          initialRole: "review",
          input: settingsInput(agent),
          catalog: { models: [], loading: false, error: "", retry: () => {} },
        }),
      ),
    );
    const picker = choices.pickers.at(-1);
    expect(picker?.modelLabel).toBe("Review model");
    expect
      .soft(picker?.models.map((row) => row.value))
      .not.toContain(INHERITED_MODEL);
    expect.soft(markup).not.toMatch(/caller[’']s model/i);
    if (model === "gpt-6-luna") {
      expect(picker?.model).toBe(model);
    } else {
      expect.soft(picker?.model).toBe("");
      expect.soft(markup).toContain("Select a review model");
      expect.soft(markup).not.toContain('aria-selected="true"');
    }
    const summary = renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(ExecutionSummaryTrigger, {
          agent,
          team: [agent],
          onOpen: () => {},
        }),
      ),
    );
    expect(summary).not.toMatch(/caller[’']s model/i);
    expect(summary).toContain(
      model === "gpt-6-luna" ? "Luna 6" : "Select a review model",
    );
    const row = renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(ExecutionSettings, {
          agentsUi: areas.agentsUi,
          settingsRow: true,
          initialRole: "review",
          input: settingsInput(agent),
          catalog: { models: [], loading: false, error: "", retry: () => {} },
        }),
      ),
    );
    expect(row).not.toMatch(/caller[’']s model/i);
    expect(row).toContain(
      model === "gpt-6-luna" ? "Luna 6" : "Select a review model",
    );
  },
);

test("worker settings retain the same-as-main model option", () => {
  renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(ExecutionSettings, {
        agentsUi: areas.agentsUi,
        presentation: "panel",
        initialRole: "worker",
        input: settingsInput({
          id: "worker-picker",
          isLead: true,
          provider: ProviderKind.codex,
          source: AgentSource.managed,
          model: "gpt-6.1-sol",
          status: AgentStatus.idle,
        }),
        catalog: { models: [], loading: false, error: "", retry: () => {} },
      }),
    ),
  );
  expect(choices.pickers.at(-1)?.models).toContainEqual(
    expect.objectContaining({
      value: INHERITED_MODEL,
      label: "Same as main agent",
    }),
  );
});

test("shows the server reason alongside an unavailable account", () => {
  const markup = renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(ExecutionSettings, {
        agentsUi: areas.agentsUi,
        presentation: "panel",
        initialRole: "orchestrator",
        input: settingsInput({
          id: "lead",
          isLead: true,
          source: AgentSource.managed,
          status: AgentStatus.idle,
          provider: ProviderKind.codex,
          accountKey: "source",
          accountDestinations: [
            {
              accountKey: "target",
              eligibility: {
                kind: "unavailable",
                reason: "Server says reconnect",
              },
            },
          ],
        }),
        accounts: [
          {
            id: "target",
            label: "Target",
            home: "",
            source: "fixture",
            provider: ProviderKind.codex,
            status: AccountStatus.error,
          },
        ],
        catalog: { models: [], loading: false, error: "", retry: () => {} },
        onAccountChange: () => {},
      }),
    ),
  );
  expect(markup).toContain("Server says reconnect");
});

test("the agent settings chooser shows exclusions even without transfer destinations", () => {
  const html = renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(ExecutionSettings, {
        agentsUi: areas.agentsUi,
        presentation: "panel",
        initialRole: "orchestrator",
        input: settingsInput({
          id: "lead",
          isLead: true,
          source: AgentSource.managed,
          status: AgentStatus.idle,
          provider: ProviderKind.codex,
          accountKey: "lead",
        }),
        accounts: [
          {
            id: "lead",
            label: "Lead",
            home: "",
            source: "fixture",
            provider: ProviderKind.codex,
            status: AccountStatus.ready,
          },
          {
            id: "other",
            label: "Other",
            home: "",
            source: "fixture",
            provider: ProviderKind.codex,
            status: AccountStatus.ready,
          },
        ],
        workerExcludedAccountKeys: ["lead"],
        reviewerExcludedAccountKeys: [],
        onWorkerExcludedAccountKeysChange: () => {},
        onReviewerExcludedAccountKeysChange: () => {},
        catalog: { models: [], loading: false, error: "", retry: () => {} },
        onAccountChange: () => {},
      }),
    ),
  );
  expect(html).not.toContain('type="checkbox"');
});

test.each(["worker", "review"] as const)(
  "%s tab reads and updates only its own exclusions",
  (role) => {
    const workers = vi.fn();
    const reviewers = vi.fn();
    choices.pickers.length = 0;
    renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(ExecutionSettings, {
          agentsUi: areas.agentsUi,
          presentation: "panel",
          initialRole: role,
          input: settingsInput({
            id: "lead",
            isLead: true,
            source: AgentSource.managed,
            status: AgentStatus.idle,
            provider: ProviderKind.codex,
            accountKey: "lead",
          }),
          accounts: [],
          workerExcludedAccountKeys: ["worker-only"],
          reviewerExcludedAccountKeys: ["review-only"],
          onWorkerExcludedAccountKeysChange: workers,
          onReviewerExcludedAccountKeysChange: reviewers,
          catalog: { models: [], loading: false, error: "", retry: () => {} },
          onAccountChange: () => {},
        }),
      ),
    );
    const picker = choices.pickers.at(-1);
    expect(picker?.excludedKeys).toEqual([
      role === "worker" ? "worker-only" : "review-only",
    ]);
    picker?.onExcludedKeysChange?.(["changed"]);
    expect(role === "worker" ? workers : reviewers).toHaveBeenCalledWith([
      "changed",
    ]);
    expect(role === "worker" ? reviewers : workers).not.toHaveBeenCalled();
  },
);

vi.mock("react", async (load) => {
  const actual = await load<typeof import("react")>();
  return {
    ...actual,
    // Server rendering exposes the real callbacks without a browser. Record
    // their state writes as a completion barrier, not a retry or elapsed wait.
    useState: (initial: unknown) => {
      const [value, write] = actual.useState(initial);
      return [
        value,
        (next: unknown) => {
          choices.states.push(next);
          write(next);
          if (next === false || (typeof next === "string" && next))
            choices.terminal?.();
        },
      ];
    },
  };
});

vi.mock("../AgentSetupPicker/AgentSetupPicker.tsx", async (load) => {
  const actual =
    await load<typeof import("../AgentSetupPicker/AgentSetupPicker.tsx")>();
  return {
    ...actual,
    AgentSetupPicker: (props: AgentSetupPickerProps) => {
      choices.pickers.push(props);
      return createElement(actual.AgentSetupPicker, props);
    },
  };
});

vi.mock("../support/WorkerModelPicker.tsx", async (load) => ({
  ...(await load<typeof import("../support/WorkerModelPicker.tsx")>()),
  useModelCatalogReader: () => ({
    scope: "future-worker-account",
    read: choices.read,
    watchResourceReads: vi.fn(),
  }),
  // The ordinary display retains stale rows. The action must use the real
  // readWorkerModels preflight instead of treating this display as admission.
  useWorkerModels: () => ({
    models: [
      {
        model: "gpt-6.1-sol",
        provider: "codex",
        serviceTiers: [],
        supportedReasoningEfforts: [{ reasoningEffort: "medium" }],
      },
      {
        model: "gpt-6-luna",
        provider: "codex",
        serviceTiers: [],
        supportedReasoningEfforts: [{ reasoningEffort: "medium" }],
      },
      {
        model: "gpt-6-astra",
        serviceTiers: capability.priority ? [{ id: "priority" }] : [],
        supportedReasoningEfforts: [],
      },
    ],
    loading: false,
    error: "",
    retry: vi.fn(),
  }),
}));

vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (load) => ({
    ...(await load<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerStore: () =>
      generatedFixtures.at(-1)?.store ?? observableFixture().store,
    useServerApi: () => ({
      post: choices.post,
      saved: (_key: string, fallback: unknown) => fallback,
      save: vi.fn(),
    }),
    useServerServices: () => null,
  }),
);

beforeEach(() => {
  stopFields?.();
  areas?.dispose();
  areas = conversationActionsFixture();
  stopFields = reaction(
    () =>
      [...areas.agentsUi.fields.values()].flatMap((state) => [
        state.saving,
        state.error,
        state.status,
      ]),
    (values) => {
      choices.states.push(...values);
      if (
        [...areas.agentsUi.fields.values()].some(
          (state) =>
            !state.saving && (state.status?.kind === "saved" || !!state.error),
        )
      )
        choices.terminal?.();
    },
  );
  for (const f of generatedFixtures.splice(0)) f.store.dispose();
  choices.pickers.length = 0;
  choices.states.length = 0;
  choices.terminal = undefined;
  choices.read.mockReset();
  choices.post.mockReset();
});

vi.mock("../../settings/models/support/modelChoices.ts", async (load) => ({
  ...(await load<
    typeof import("../../settings/models/support/modelChoices.ts")
  >()),
  useModelChoices: () => ({}),
}));

for (const initialRole of ["orchestrator", "worker", "review"] as const) {
  for (const priority of [true, false]) {
    test(`${initialRole} Fast mode follows the selected model capability (${priority})`, () => {
      capability.priority = priority;
      const agent: Agent = {
        id: "fast-picker",
        isLead: true,
        source: AgentSource.managed,
        status: AgentStatus.idle,
        model: "gpt-6-astra",
        workerDefaults: { model: "gpt-6-astra", effort: null, fastMode: false },
        reviewDefaults: { model: "gpt-6-astra", effort: null },
      };
      const markup = renderToStaticMarkup(
        createElement(
          MantineProvider,
          null,
          createElement(ExecutionSettings, {
            agentsUi: areas.agentsUi,
            presentation: "panel",
            initialRole,
            input: settingsInput(agent),
            catalog: {
              models: [
                {
                  model: "gpt-6-astra",
                  serviceTiers: priority ? [{ id: "priority" }] : [],
                  supportedReasoningEfforts: [],
                },
              ],
              loading: false,
              error: "",
              retry: () => {},
            },
          }),
        ),
      );
      expect(markup.includes('aria-label="Fast mode"')).toBe(priority);
      if (initialRole === "review") {
        expect(markup).not.toContain('aria-label="Daybreak"');
        expect(markup).toContain("review tasks use Workers");
      }
    });
  }
}

for (const isLead of [true, false]) {
  test(`the ${isLead ? "lead" : "worker"} panel always renders every role`, () => {
    const markup = renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(ExecutionSettings, {
          agentsUi: areas.agentsUi,
          presentation: "panel",
          settingsRow: true,
          input: settingsInput({
            id: "picker-test",
            name: "Test",
            isLead,
            source: AgentSource.managed,
            model: "gpt-6-astra",
            status: AgentStatus.idle,
          }),
          catalog: { models: [], loading: false, error: "", retry: () => {} },
        }),
      ),
    );
    for (const label of ["Lead", "Workers", "Review"]) {
      expect(markup).toContain(`>${label}</span></label>`);
    }
    expect(markup.match(/aria-label="Agent role"/g)).toHaveLength(1);
  });
}

for (const initialRole of ["orchestrator", "review"] as const) {
  test(`a worker without its lead keeps ${initialRole} visible as unavailable`, () => {
    const markup = renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(ExecutionSettings, {
          agentsUi: areas.agentsUi,
          presentation: "panel",
          initialRole,
          input: settingsInput({
            id: "worker",
            name: "Worker",
            isLead: false,
            source: AgentSource.managed,
            model: "gpt-6-luna",
            status: AgentStatus.idle,
          }),
          catalog: { models: [], loading: false, error: "", retry: () => {} },
        }),
      ),
    );
    expect(markup).toContain(
      "Managed by the lead. Its settings are unavailable in this chat.",
    );
    expect(markup).toContain("Inherited settings unavailable");
    expect(markup).toContain('aria-disabled="true"');
    expect(markup).not.toContain('data-value="gpt-6-luna"');
    for (const label of ["Lead", "Workers", "Review"])
      expect(markup).toContain(`>${label}</span></label>`);
  });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

const freshModels = {
  data: [
    {
      model: "gpt-6.1-sol",
      provider: "codex",
      isDefault: true,
      defaultReasoningEffort: "medium",
      supportedReasoningEfforts: [{ reasoningEffort: "medium" }],
      serviceTiers: [],
    },
    {
      model: "gpt-6-luna",
      provider: "codex",
      supportedReasoningEfforts: [{ reasoningEffort: "medium" }],
      serviceTiers: [],
    },
  ],
};

function futureWorkerChoice() {
  const agent = {
    id: "pm-installation",
    rootId: "pm-installation",
    name: "PM installation",
    accountKey: "lead-claude",
    provider: ProviderKind.claude,
    model: "default",
    isLead: true,
    source: AgentSource.managed,
    status: AgentStatus.waiting,
    workerDefaults: {
      model: "gpt-6.1-sol",
      effort: "medium",
      fastMode: false,
      daybreakEnabled: false,
      delegationDepth: 1,
    },
  } satisfies Agent;
  const children = [
    { id: "existing-child", rootId: agent.id, accountKey: "old-codex" },
  ];
  const originalChildren = structuredClone(children);
  const done = deferred<void>();
  choices.terminal = () => done.resolve();
  choices.post.mockImplementation(async (path, request) => {
    expect(path).toBe("/api/conversation");
    expect(request.expected_account_key).toBe("lead-claude");
    return {
      ...agent,
      workerDefaults: {
        ...agent.workerDefaults,
        accountKey: request.worker_defaults.account_key,
      },
    };
  });
  renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(ExecutionSettings, {
        agentsUi: areas.agentsUi,
        presentation: "panel",
        initialRole: "worker",
        settingsRow: true,
        input: settingsInput(agent),
        team: children,
        accounts: [
          {
            id: "lead-claude",
            label: "Lead",
            home: "/fixture/lead-claude",
            source: "fixture",
            provider: ProviderKind.claude,
            status: AccountStatus.ready,
          },
          {
            id: "old-codex",
            label: "Old",
            home: "/fixture/old-codex",
            source: "fixture",
            provider: ProviderKind.codex,
            status: AccountStatus.ready,
          },
          {
            id: "future-codex",
            label: "Future",
            home: "/fixture/future-codex",
            source: "fixture",
            provider: ProviderKind.codex,
            status: AccountStatus.ready,
          },
        ],
        catalog: { models: [], loading: false, error: "", retry: vi.fn() },
      }),
    ),
  );
  expect(choices.pickers).toHaveLength(1);
  const picker = choices.pickers[0];
  if (!picker) throw new Error("ExecutionSettings did not render its picker");
  return {
    picker,
    agent,
    children,
    originalChildren,
    done,
  };
}

test("worker settings open on the saved model provider", () => {
  const { picker } = futureWorkerChoice();
  expect(picker.provider).toBe("codex");
});

test("the actual future-worker model choice waits for fresh metadata", async () => {
  const gate = deferred<typeof freshModels>();
  choices.read.mockReturnValue(gate.promise);
  const fixture = futureWorkerChoice();
  fixture.picker.onModel("gpt-6-luna");
  try {
    expect(choices.read).toHaveBeenCalledExactlyOnceWith({
      query: {
        account_key: "lead-claude",
        workers: "1",
        fresh: "1",
      },
    });
    expect(choices.post).not.toHaveBeenCalled();
    expect(fixture.children).toEqual(fixture.originalChildren);
  } finally {
    gate.resolve(freshModels);
    await fixture.done.promise;
  }
  expect(choices.post).toHaveBeenCalledOnce();
  expect(choices.post.mock.calls[0]).toEqual([
    "/api/conversation",
    {
      id: "pm-installation",
      expected_account_key: "lead-claude",
      worker_defaults: {
        model: "gpt-6-luna",
        effort: "medium",
        fast_mode: false,
        daybreak_enabled: false,
        delegation_depth: 1,
      },
    },
    { timeoutMs: 15000 },
  ]);
  expect(fixture.children).toEqual(fixture.originalChildren);
});

test("a fresh catalog cannot silently replace the requested future-worker model", async () => {
  const gate = deferred<typeof freshModels>();
  choices.read.mockReturnValue(gate.promise);
  const fixture = futureWorkerChoice();
  fixture.picker.onModel("gpt-6-luna");
  gate.resolve({ data: freshModels.data.slice(0, 1) });
  await fixture.done.promise;
  expect(choices.post).not.toHaveBeenCalled();
  expect(choices.states).toContain(
    "This model is not available for the selected worker accounts.",
  );
  expect(fixture.agent.workerDefaults.model).toBe("gpt-6.1-sol");
  expect(fixture.children).toEqual(fixture.originalChildren);
});

for (const provider of ["claude", "future"] as const) {
  test(`the lead settings gate model and reasoning by the ${provider} chat provider`, () => {
    renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(ExecutionSettings, {
          agentsUi: areas.agentsUi,
          presentation: "panel",
          initialRole: "orchestrator",
          input: settingsInput({
            id: "capability-gate",
            name: "Lead",
            isLead: true,
            // "future" is not a known provider on purpose: it checks the unknown-provider path.
            provider: provider as never,
            source: AgentSource.managed,
            model: "gpt-6-astra",
            status: AgentStatus.idle,
          }),
          catalog: { models: [], loading: false, error: "", retry: () => {} },
        }),
      ),
    );
    // The picker receives the chat's provider; AgentSetupPicker.test.ts covers hiding.
    expect(choices.pickers.at(-1)?.capabilityProvider).toBe(provider);
  });
}

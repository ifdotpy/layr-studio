import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import { AccountStatus, ProviderKind } from "../../../generated/api.ts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { expect, test } from "vitest";
import {
  AgentSetupPicker,
  type AgentSetupPickerProps,
} from "./AgentSetupPicker.tsx";

const base: AgentSetupPickerProps = {
  role: "orchestrator",
  onRole: () => undefined,
  provider: "claude",
  onProvider: () => undefined,
  providers: [],
  accounts: [],
  accountKey: "",
  onAccount: () => undefined,
  models: [],
  model: "",
  onModel: () => undefined,
  modelLabel: "Test model",
  efforts: [{ value: "high", label: "High" }],
  effort: "high",
  onEffort: () => undefined,
  effortLabel: "Test reasoning",
  disabled: false,
  fastAvailable: false,
  fast: false,
  onFast: () => undefined,
  daybreakAvailable: false,
  daybreak: false,
  onDaybreak: () => undefined,
};

function render(patch: Partial<AgentSetupPickerProps>): string {
  return renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(
        ServerStoreProvider,
        { value: observableFixture().store },
        createElement(AgentSetupPicker, { ...base, ...patch }),
      ),
    ),
  );
}

test("a provider without model or effort support hides both fields", () => {
  const markup = render({ provider: "future" });
  expect(markup).not.toContain("Test model");
  expect(markup).not.toContain("Test reasoning");
});

test("a provider with model and effort support shows both fields", () => {
  const markup = render({ provider: "claude" });
  expect(markup).toContain("Test model");
  expect(markup).toContain("Test reasoning");
});

test("capabilityProvider gates the fields instead of the picked provider", () => {
  expect(
    render({ provider: "claude", capabilityProvider: "future" }),
  ).not.toContain("Test reasoning");
  expect(
    render({ provider: "future", capabilityProvider: "claude" }),
  ).toContain("Test reasoning");
});

test("no provider picked yet keeps both fields visible", () => {
  const markup = render({ provider: "" });
  expect(markup).toContain("Test model");
  expect(markup).toContain("Test reasoning");
});

test("workers can browse Claude models with a Codex lead", () => {
  const markup = render({
    role: "worker",
    provider: "codex",
    providers: ["codex", "claude"],
    showAccountChooser: false,
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
        id: "claude",
        label: "Claude",
        home: "",
        source: "fixture",
        provider: ProviderKind.claude,
        status: AccountStatus.ready,
      },
    ],
    excludedKeys: [],
    onExcludedKeysChange: () => {},
  });
  expect(markup).toContain('aria-label="Subagent account provider"');
  expect(markup).toContain("Claude");
  expect(markup).toContain("Use Lead for workers");
});

test("the lead has no account checkboxes and reviewers have their own", () => {
  const accounts: AgentSetupPickerProps["accounts"] = [
    {
      id: "one",
      label: "One",
      home: "",
      source: "fixture",
      provider: ProviderKind.codex,
      status: AccountStatus.ready,
    },
  ];
  const props = {
    provider: "codex",
    accounts,
    excludedKeys: [],
    onExcludedKeysChange: () => {},
  };
  expect(render({ ...props, role: "orchestrator" })).not.toContain(
    "Use One for",
  );
  const reviewer = render({ ...props, role: "review" });
  expect(reviewer).toContain("Use One for reviewers");
  expect(reviewer).toContain("disabled");
});

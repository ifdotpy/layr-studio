import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { expect, test } from "vitest";
import {
  AgentSetupPicker,
  type AgentSetupPickerProps,
} from "./AgentSetupPicker";

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
      createElement(AgentSetupPicker, { ...base, ...patch }),
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

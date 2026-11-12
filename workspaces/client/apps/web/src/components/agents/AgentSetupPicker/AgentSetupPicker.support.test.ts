// @vitest-environment happy-dom
import { act, createElement, type ComponentProps } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, test, vi } from "vitest";
import { AgentSetupPicker } from "./AgentSetupPicker.tsx";

vi.mock("../../accounts/AccountTiles/AccountTiles.tsx", () => ({
  AccountTiles: () => null,
}));
vi.mock("../../settings/models/ModelField/ModelField.tsx", () => ({
  ModelField: () => null,
}));
vi.mock("../../settings/models/EffortField/EffortField.tsx", () => ({
  EffortField: () => null,
}));
vi.mock("../../settings/models/ModeToggles/ModeToggles.tsx", () => ({
  ModeToggles: () => null,
}));

test("setup displays the generated unsupported reason and clears it for a supported provider", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"),
    root = createRoot(host);
  const props = {
    role: "orchestrator",
    onRole: vi.fn(),
    provider: "claude",
    onProvider: vi.fn(),
    providers: [],
    accounts: [],
    accountKey: "",
    onAccount: vi.fn(),
    models: [],
    model: "",
    onModel: vi.fn(),
    modelLabel: "Model",
    efforts: [],
    effort: "",
    onEffort: vi.fn(),
    effortLabel: "Effort",
    disabled: false,
    fastAvailable: false,
    fast: false,
    onFast: vi.fn(),
    daybreakAvailable: false,
    daybreak: false,
    onDaybreak: vi.fn(),
  } satisfies ComponentProps<typeof AgentSetupPicker>;
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(AgentSetupPicker, props),
        ),
      ),
    );
    expect(host.textContent).toContain("Claude settings are session scoped.");
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(AgentSetupPicker, { ...props, provider: "codex" }),
        ),
      ),
    );
    expect(host.textContent).not.toContain(
      "Claude settings are session scoped.",
    );
  } finally {
    await act(async () => root.unmount());
    vi.unstubAllGlobals();
  }
});

import { ProviderKind } from "../../../generated/api.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, expect, it, vi } from "vitest";
import { observable, action } from "mobx";
import * as api from "../../../servers/net/api.ts";
import type { PostResult } from "../../../api.ts";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import { ClaudeSettings } from "../ClaudeSettings/ClaudeSettings.tsx";
import { ClaudeSettingsStore } from "./ClaudeSettingsStore.ts";

vi.mock("../../agents/support/WorkerModelPicker.tsx", () => ({
  useWorkerModels: () => ({ models: [], loading: false, error: "" }),
}));
afterEach(() => vi.unstubAllGlobals());

it("loads only while the rendered settings are observed and keeps account events from resetting edits", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const selection = observable({
    id: "lead",
    accountKey: "default",
    permissionMode: "default" as const,
  });
  const ownedApi = { ...api };
  const post = vi.spyOn(ownedApi, "post").mockResolvedValue({
    settings: { permissionMode: "acceptEdits" },
    tasks: [],
    turns: [],
  });
  const model = new ClaudeSettingsStore(ownedApi, () => selection);
  const fixture = observableFixture([
    { id: "lead", provider: ProviderKind["claude"] },
  ]);
  const agent = fixture.mirror.current.agents.get("lead");
  if (!agent) throw new Error("Missing fixture agent");
  const host = document.createElement("div"),
    root = createRoot(host);
  expect(post).not.toHaveBeenCalled();
  try {
    await act(async () =>
      root.render(
        createElement(
          fixture.Provider,
          null,
          createElement(
            MantineProvider,
            null,
            createElement(ClaudeSettings, {
              agent,
              model,
            }),
          ),
        ),
      ),
    );
    expect(post).toHaveBeenCalledExactlyOnceWith(
      "/api/claude/session",
      { id: "lead", action: "state" },
      { timeoutMs: 15000 },
    );
    expect(model.permissionMode).toBe("acceptEdits");
    expect(
      host
        .querySelector('section[aria-label="Claude settings"]')
        ?.getAttribute("aria-busy"),
    ).toBe("false");
    await act(async () => {
      model.setCommand("/local-edit");
      action(() => {
        selection.accountKey = "other";
      })();
    });
    expect(model.command).toBe("/local-edit");
    expect(post).toHaveBeenCalledTimes(1);
  } finally {
    await act(async () => root.unmount());
    model.dispose();
    fixture.store.dispose();
  }
});

it("does not accept an old selected chat's state response", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const selection = observable({ id: "first" });
  let finish!: (result: PostResult<"/api/claude/session">) => void;
  const ownedApi = { ...api };
  const post = vi
    .spyOn(ownedApi, "post")
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValue({
      settings: { permissionMode: "acceptEdits" },
      tasks: [],
      turns: [],
    });
  const model = new ClaudeSettingsStore(ownedApi, () => selection);
  const fixture = observableFixture([
    { id: "first", provider: ProviderKind["claude"] },
  ]);
  const agent = fixture.mirror.current.agents.get("first");
  if (!agent) throw new Error("Missing fixture agent");
  const host = document.createElement("div"),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          fixture.Provider,
          null,
          createElement(
            MantineProvider,
            null,
            createElement(ClaudeSettings, { agent, model }),
          ),
        ),
      ),
    );
    await act(async () => {
      action(() => {
        selection.id = "second";
      })();
    });
    await act(async () => {
      finish({ settings: { permissionMode: "default" }, tasks: [], turns: [] });
    });
    expect(model.permissionMode).toBe("acceptEdits");
    expect(post).toHaveBeenCalledTimes(2);
  } finally {
    await act(async () => root.unmount());
    model.dispose();
    fixture.store.dispose();
  }
});

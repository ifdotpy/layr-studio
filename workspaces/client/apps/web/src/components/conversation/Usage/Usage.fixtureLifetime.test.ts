import { getObserverTree } from "mobx";
import { resourceData } from "../../../store/runtime.ts";
import * as api from "../../../servers/net/api.ts";
// @vitest-environment happy-dom
import { act, createElement, StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import * as usage from "./Usage.tsx";
import {
  UsageFixture,
  createUsageFixture,
} from "../../../app/settings/UsageFixture.tsx";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";

it("leaves no fixture reaction alive after Strict Mode discards an initializer", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const models = observableFixture([{ id: "fixture" }]);
  const created: ReturnType<typeof usage.createUsageView>[] = [];
  const create = usage.createUsageView;
  const spy = vi
    .spyOn(usage, "createUsageView")
    .mockImplementation((...args) => {
      const owner = create(...args);
      created.push(owner);
      return owner;
    });
  const fixture = createUsageFixture(models.store, api, {
    agent: { id: "fixture" },
  });
  fixture.start();
  const host = document.createElement("div"),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          StrictMode,
          null,
          createElement(
            MantineProvider,
            { env: "test" },
            createElement(
              models.Provider,
              null,
              createElement(UsageFixture, {
                agent: { id: "fixture" },
                owner: fixture,
              }),
            ),
          ),
        ),
      ),
    );
    await act(async () =>
      root.render(
        createElement(
          StrictMode,
          null,
          createElement(
            MantineProvider,
            { env: "test" },
            createElement(UsageFixture, {
              agent: { id: "fixture" },
              owner: fixture,
            }),
          ),
        ),
      ),
    );
    expect(created).toHaveLength(1);
    await act(async () => root.unmount());
    const source = resourceData(fixture.store.state().current)?.accountsSummary;
    if (!source) throw new Error("Fixture account source missing");
    expect(
      getObserverTree(source, "current").observers?.length,
    ).toBeGreaterThan(0);
    fixture.dispose();
    expect(getObserverTree(source, "current").observers ?? []).toHaveLength(0);
    expect(created.length).toBeGreaterThan(0);
    expect(created.every((owner) => !owner.active)).toBe(true);
    expect(created.every((owner) => owner.interests.size === 0)).toBe(true);
  } finally {
    fixture.dispose();
    for (const owner of created) owner.dispose();
    models.store.dispose();
    spy.mockRestore();
    vi.unstubAllGlobals();
  }
});

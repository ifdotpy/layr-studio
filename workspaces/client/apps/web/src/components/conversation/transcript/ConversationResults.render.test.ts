// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { observable, runInAction } from "mobx";
import { MantineProvider } from "@mantine/core";
import { observableFixture } from "../../observableFixture.tsx";
import { ServerStoreProvider } from "../../../servers/ServerServicesContext.tsx";
import { expect, it, vi } from "vitest";
import ConversationResults from "./ConversationResults";
import type { Message } from "../../../types";

const header = vi.hoisted(() => ({ renders: 0 }));
vi.mock("../../ui/primitives", () => ({
  PanelHeader: () => {
    header.renders++;
    return null;
  },
}));

it("unchanged results of a stable group do not rerender when a message arrives", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const messages = observable.array<Message>([
    { id: "file", role: "assistant", text: "[readme](./README.md)" },
  ]);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const models = observableFixture();
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            ServerStoreProvider,
            { value: models.store },
            createElement(ConversationResults, { messages, agentId: "agent" }),
          ),
        ),
      ),
    );
    expect(host.textContent).toContain("readme");
    const before = header.renders;
    await act(async () =>
      runInAction(() => {
        messages.push({ id: "plain", role: "user", text: "continue" });
      }),
    );
    expect(header.renders).toBe(before);
  } finally {
    await act(async () => root.unmount());
    models.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});

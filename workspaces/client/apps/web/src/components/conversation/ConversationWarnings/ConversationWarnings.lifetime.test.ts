// @vitest-environment happy-dom
import { act, createElement as h } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { action } from "mobx";
import { expect, it, vi } from "vitest";
import ConversationWarnings from "./ConversationWarnings.tsx";
import { ServerAreasContextProvider as ConversationProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { ConversationStore } from "../support/ConversationStore.ts";
import { createConversationContext } from "../support/conversationContext.ts";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../shell/integration-support/observableFixture.tsx";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import { DisclosureState } from "../../../stores/DisclosureState.ts";
import { AgentSource } from "../../../generated/api.ts";
import { requestTestEnvironment } from "../questions/Requests/requestTestFixture.ts";

requestTestEnvironment();
it.each(["owner replacement", "account change"])(
  "respects warning lifetime after %s",
  async (change) => {
    const models = observableFixture([
      { id: "lead", source: AgentSource.managed, accountKey: "one" },
    ]);
    const areas = conversationActionsFixture(vi.fn(), models.store);
    action(() => {
      areas.navigation.selectedId = "lead";
    })();
    const create = () =>
      new ConversationStore(
        createConversationContext(areas, areas.selected.attachmentDrafts),
      );
    const first = create(),
      second = create();
    const obsoleteDisclosure = new DisclosureState();
    Object.defineProperty(first, "warnings", { value: obsoleteDisclosure });
    action(() => {
      areas.selected.conversation = first;
    })();
    const stopFirst = first.start(),
      stopSecond = second.start();
    let firstStarted = true;
    const host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);
    const render = () =>
      root.render(
        h(
          MantineProvider,
          { env: "test" },
          h(ConversationProvider, { value: areas }, h(ConversationWarnings)),
        ),
      );
    const open = () =>
      host
        .querySelector<HTMLButtonElement>(".conversation-warning-trigger")!
        .click();
    try {
      await act(async () => render());
      await act(async () => open());
      expect(document.querySelector('[role="dialog"]')).not.toBeNull();
      if (change === "owner replacement") {
        await act(async () =>
          action(() => {
            areas.selected.conversation = second;
            stopFirst();
            firstStarted = false;
            // Disposed owner state must not control the still-mounted local modal.
            obsoleteDisclosure.close();
          })(),
        );
        expect(document.querySelector('[role="dialog"]')).not.toBeNull();
        await act(async () =>
          document
            .querySelector<HTMLButtonElement>(".mantine-Modal-close")!
            .click(),
        );
        expect(document.querySelector('[role="dialog"]')).toBeNull();
        await act(async () => open());
        expect(document.querySelector('[role="dialog"]')).not.toBeNull();
        expect(obsoleteDisclosure.opened).toBe(false);
        return;
      }
      await act(async () =>
        models.apply([
          {
            collection: "agentSummary",
            id: "lead",
            value: agentSummaryFixture("lead", {
              source: AgentSource.managed,
              accountKey: "two",
            }),
          },
        ]),
      );
      expect(document.querySelector('[role="dialog"]')).toBeNull();
    } finally {
      await act(async () => root.unmount());
      if (firstStarted) stopFirst();
      stopSecond();
      areas.scope.services.close();
      models.store.dispose();
      host.remove();
    }
  },
);

import "../../src/rendererStyles.ts";
import { fixtureElement } from "./browser-helpers.ts";

import React, { useEffect } from "react";
import { createRoot } from "react-dom/client";
import { useStartupFixture } from "./startup-fixture.ts";
import { createDrafts } from "../../src/sync/drafts.ts";
import * as api from "../../src/api.ts";
import * as resources from "../../src/sync/resourceEvents.ts";
import {
  subscribeTranscriptProjection,
  watchResourceChanges,
  prefetchTranscript,
} from "../../src/sync/client.ts";
import { durableSend } from "../../src/sync/send.ts";
window.prefetchTranscript = prefetchTranscript;
window.durableSend = durableSend;
window.mount = (selected) =>
  createRoot(fixtureElement("app")).render(
    React.createElement(function Fixture() {
      window.syncHttpConnectionBudgetBrowserFixtureSnapshot =
        useStartupFixture();
      useEffect(() => createDrafts({ api, resources }).start(), []);
      useEffect(
        () =>
          subscribeTranscriptProjection(
            `transcript:${selected}`,
            () => {},
            () => {},
          ),
        [],
      );
      useEffect(
        () =>
          window.syncHttpConnectionBudgetBrowserFixtureSnapshot.startup
            .workspaceId
            ? watchResourceChanges(
                { kind: "transcript", agentId: selected },
                () => {},
              )
            : undefined,
        [
          window.syncHttpConnectionBudgetBrowserFixtureSnapshot.startup
            .workspaceId,
        ],
      );
      return React.createElement(
        "div",
        { id: "error" },
        window.syncHttpConnectionBudgetBrowserFixtureSnapshot.startup.error,
      );
    }),
  );

declare global {
  interface Window {
    prefetchTranscript: typeof import("../../src/sync/client.ts").prefetchTranscript;
    durableSend: typeof import("../../src/sync/send.ts").durableSend;
    syncHttpConnectionBudgetBrowserFixtureSnapshot: import("./browser-helpers.ts").FixtureSnapshot;
    mount: (selected: string) => void;
    refresh: Promise<unknown>;
    warm: Promise<unknown>;
  }
}

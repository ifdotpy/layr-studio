import { observer } from "mobx-react-lite";
import "../../src/rendererStyles.ts";
import {
  PublishedAgentFixture,
  usePublishedAgent,
  publishFixtureAgent,
} from "../fixtures/PublishedAgentFixture.tsx";
import {
  initialConfirmedLead,
  confirmedWorkspaceId,
} from "./concurrency-fixture-data.ts";
import type { Agent } from "../../src/types.ts";
declare global {
  interface Window {
    refreshCount: number;
    refreshHold: boolean;
    refreshFailure: boolean;
    refreshRelease?: () => void;
    setLead?: (lead: Agent) => void;
  }
}
import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import Control from "../../src/components/agents/SubagentConcurrencyControl/SubagentConcurrencyControl.tsx";
window.refreshCount = 0;
window.refreshHold = true;
window.refreshFailure = false;
const Fixture = observer(function Fixture() {
  const lead = usePublishedAgent(initialConfirmedLead.id, initialConfirmedLead);
  window.setLead = publishFixtureAgent;
  return (
    <MantineProvider>
      <Control
        compact
        lead={lead}
        stateDir="fixture-state"
        workspaceId={confirmedWorkspaceId}
      />
    </MantineProvider>
  );
});
await publishFixtureAgent(initialConfirmedLead);
createRoot(document.getElementById("root")!).render(
  <PublishedAgentFixture>
    <Fixture />
  </PublishedAgentFixture>,
);

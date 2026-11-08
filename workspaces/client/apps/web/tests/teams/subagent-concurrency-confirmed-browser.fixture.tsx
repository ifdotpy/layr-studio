import "../../src/rendererStyles.ts";
import { AgentModelsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
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
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import Control from "../../src/components/agents/SubagentConcurrencyControl/SubagentConcurrencyControl.tsx";
window.refreshCount = 0;
window.refreshHold = true;
window.refreshFailure = false;
function Fixture() {
  const [lead, setLead] = useState<Agent>(initialConfirmedLead);
  window.setLead = (value) => flushSync(() => setLead(value));
  return (
    <MantineProvider>
      <AgentModelsFixture agents={[lead]}>
        <Control
          compact
          lead={lead}
          stateDir="fixture-state"
          workspaceId={confirmedWorkspaceId}
          refresh={async () => {
            window.refreshCount++;
            if (window.refreshHold)
              await new Promise<void>(
                (resolve) => (window.refreshRelease = resolve),
              );
            if (window.refreshFailure)
              throw new Error("Credentials unavailable");
          }}
        />
      </AgentModelsFixture>
    </MantineProvider>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);

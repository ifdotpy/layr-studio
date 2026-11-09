import { observer } from "mobx-react-lite";
import "../../src/rendererStyles.ts";
import {
  PublishedAgentFixture,
  usePublishedAgent,
  publishFixtureAgent,
} from "../fixtures/PublishedAgentFixture.tsx";
import type { Agent } from "../../src/types.ts";
import { original } from "./disconnect-recovery-ui.data.ts";
import { fixtureElement } from "./browser-helpers.ts";

import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import { NativeError } from "../../src/components/shell/NativeNotice/NativeNotice.tsx";
import { setToken } from "../../src/api.ts";
setToken("fixture-token");
window.disconnectRecoveryUiFixtureRefreshes = [];
const Harness = observer(function Harness() {
  const [selectedId, setSelectedId] = useState(readAgent().id);
  const agent = usePublishedAgent(selectedId, readAgent());
  window.disconnectRecoveryUiFixtureSetAgent = async (value) => {
    const next = typeof value === "function" ? value(agent) : value;
    setSelectedId(next.id);
    await publishFixtureAgent(next);
  };
  return (
    <MantineProvider>
      <div data-agent={agent.id}>
        <NativeError agent={agent} />
        <p data-history>Previously recorded conversation remains available.</p>
      </div>
    </MantineProvider>
  );
});
await publishFixtureAgent(readAgent());
createRoot(fixtureElement("root")).render(
  <PublishedAgentFixture>
    <Harness />
  </PublishedAgentFixture>,
);

declare global {
  interface Window {
    disconnectRecoveryUiFixtureRefreshes: string[];
    disconnectRecoveryUiFixtureSetAgent: import("react").Dispatch<
      import("react").SetStateAction<import("../../src/types.ts").Agent>
    >;
  }
}

// This fixture restores only the agent document it wrote itself.
function readAgent(): Agent {
  return (
    (JSON.parse(
      localStorage.getItem("fixture-agent") || "null",
    ) as Agent | null) || original
  );
}

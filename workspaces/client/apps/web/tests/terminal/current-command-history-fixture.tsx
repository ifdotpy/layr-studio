import "../../src/rendererStyles.ts";
import { AgentModelsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { ComponentProps } from "react";
import type { Message } from "../../src/types.ts";
export type HistorySnapshot = {
  label: string;
  items: Message[];
  enabled: boolean;
  currentTurn?: string | undefined;
};
declare global {
  interface Window {
    applyHistorySnapshot: (snapshot: HistorySnapshot) => void;
  }
}

import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import History from "../../src/components/conversation/transcript/TurnHistory/TurnHistory.fixture.tsx";
function TurnHistory(props: ComponentProps<typeof History>) {
  return (
    <AgentModelsFixture agents={props.agent ? [props.agent] : []}>
      <History {...props} />
    </AgentModelsFixture>
  );
}
function Fixture() {
  const [snapshot, setSnapshot] = useState<HistorySnapshot | null>(null);
  window.applyHistorySnapshot = setSnapshot;
  return (
    <MantineProvider>
      <main data-case={snapshot?.label}>
        {snapshot && (
          <TurnHistory
            key={snapshot.label}
            items={snapshot.items}
            currentTurn={snapshot.currentTurn}
            enabled={snapshot.enabled}
            storageKey="fixture"
            renderMessage={(item) => <p data-message={item.id}>{item.text}</p>}
            onJump={() => {}}
          />
        )}
        <output hidden>{JSON.stringify(snapshot?.items)}</output>
      </main>
    </MantineProvider>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);

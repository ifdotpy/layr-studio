import "../../src/rendererStyles.ts";
import type { ComponentSnapshot as Snapshot } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { messageModels } from "../messages/message-models.fixture";
import { conversationActionsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { Agent, Room } from "../../src/types.ts";
import { fixtureSnapshot } from "../fixtures/snapshot.ts";
declare global {
  interface Window {
    showCurrent: () => void;
  }
}

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import TeamChats from "../../src/components/shell/messages/TeamChats/TeamChats.tsx";
import "../../src/style.css";
import "../../src/workspace-layout.css";
import "../../src/appearance.css";
import { theme } from "../../src/theme.ts";
const lead: Agent = {
  id: "lead",
  name: "Lead",
  rootId: "lead",
  isLead: true,
  source: "managed",
  created: 1,
  updated: 1,
};
const worker: Agent = {
  id: "worker",
  name: "Worker",
  rootId: "lead",
  isLead: false,
  source: "managed",
  created: 1,
  updated: 1,
};
const room: Room = {
  id: "private:lead:worker",
  kind: "private",
  members: ["lead", "worker"],
  name: "Worker",
  updated: 1,
  lastMessage: {
    seq: 1391,
    text: "room message 1391",
    created: 1,
    sender: "worker",
  },
};
const data: Snapshot = {
  ...fixtureSnapshot([]),
  stateDir: "room-fixture",
  threads: [lead, worker],
  runtime: {
    ...fixtureSnapshot([]).runtime,
    rooms: [room],
    requests: [],
    complaints: [],
    projects: [],
    peerTeams: [],
  },
};
const models = messageModels(data.stateDir, data.threads);
models.apply([{ collection: "room", id: room.id, value: room }]);
const app = createRoot(document.getElementById("root")!);
window.showCurrent = () =>
  app.render(
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <models.Provider commands={conversationActionsFixture()}>
        <TeamChats leadId="lead" />
      </models.Provider>
    </MantineProvider>,
  );
window.showCurrent();

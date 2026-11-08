import "../../src/rendererStyles.ts";
import type { Complaint } from "../../src/types.ts";

import React from "react";
import { messageModels } from "./message-models.fixture";
import { conversationActionsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import UserMessages from "../../src/components/shell/messages/UserMessages/UserMessages.tsx";
const models = messageModels("ownership-contract", [
  { id: "lead", name: "Release lead" },
  { id: "worker", name: "Worker" },
]);
const root = createRoot(document.getElementById("root")!);
let records: Complaint[] = [],
  target: "user" | "lead" = "user";
const render = () => {
  models.apply(
    records.map((value) => ({ collection: "complaint", id: value.id, value })),
  );
  return root.render(
    React.createElement(
      MantineProvider,
      null,
      React.createElement(
        models.Provider,
        { commands: conversationActionsFixture() },
        React.createElement(UserMessages, { target }),
      ),
    ),
  );
};
window.setRecords = (value) => {
  records = value;
  render();
};
window.setTarget = (value) => {
  target = value;
  render();
};

declare global {
  interface Window {
    setRecords: (value: Complaint[]) => void;
    setTarget: (value: "user" | "lead") => void;
    wrongMessage: boolean;
    observer: MutationObserver;
  }
}

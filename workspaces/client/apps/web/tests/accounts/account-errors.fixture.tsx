import "../../src/rendererStyles.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import AccountSignIn from "../../src/components/accounts/AccountSignIn/AccountSignIn.tsx";
import Workspace from "../../src/components/shell/Workspace/Workspace.tsx";
import type { Agent } from "../../src/types.ts";
import { accountErrorFailure } from "./account-errors-data.ts";
import {
  createAuthProgressMirror,
  serializeStateCollections,
} from "../../src/generated/observableResources.ts";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
const { negative }: { negative: boolean } = await (
  await fetch("/fixture-data")
).json();
const agent = {
  id: "lead",
  rootId: "lead",
  isLead: true,
  source: "managed",
  status: "idle",
  model: "gpt-6",
  name: "Fixture lead",
  created: 1,
  cwd: "/fixture",
} satisfies Agent;
localStorage.setItem("account-sign-in:fixture", JSON.stringify("request"));
declare global {
  interface Window {
    showSurface(surface: string): void;
  }
}
function Harness() {
  const [surface, setSurface] = useState("signin");
  window.showSurface = setSurface;
  return (
    <MantineProvider>
      <p data-shell>Saved conversation remains available</p>
      {surface === "signin" ? (
        <AccountSignIn scope="fixture" />
      ) : (
        <Workspace
          key={surface}
          opened
          initialSection={surface}
          agent={agent}
          onClose={() => window.showSurface("signin")}
          onSelect={() => {}}

          notify={() => {}}
        />
      )}
    </MantineProvider>
  );
}
const fixture = observableFixture([agent]);
fixture.apply([
  ...serializeStateCollections(fixture.mirror.current),
  {
    collection: "accounts",
    id: "current",
    value: {
      accounts: [
        {
          id: "default",
          label: "Fixture account",
          home: "/fixture",
          source: "fixture",
          provider: "codex",
          status: "ready",
        },
      ],
      archivedAccounts: [],
      defaultAccountGroups: [],
      defaultAccountKey: "default",
      supportsDelete: true,
      supportsDisconnect: true,
    },
  },
]);
const receipt = createAuthProgressMirror({
  requestId: "request",
  accountKey: "default",
  status: "error",
  // @ts-expect-error Negative control deliberately tests a structured error outside the wire string contract.
  error: negative ? accountErrorFailure : accountErrorFailure.message,
});
const store = {
  ...fixture.store,
  authProgress: () => ({
    current: { status: "live" as const, data: receipt.current },
  }),
};
createRoot(document.getElementById("root")!).render(
  <ServerStoreProvider value={store}>
    <Harness />
  </ServerStoreProvider>,
);

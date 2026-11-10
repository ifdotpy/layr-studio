import "../../src/rendererStyles.ts";
import {
  conversationActionsFixture,
  observableFixture,
} from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { AccountsState } from "../../src/components/accounts/support/Accounts.tsx";
import type { ServerCommand } from "../../src/servers/navigation.ts";
import { useState } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { MantineProvider } from "@mantine/core";
import Accounts, {
  frameAccounts,
} from "../../src/components/accounts/Accounts/Accounts.tsx";
import ServerAccountsPanel from "../../src/components/servers/ServerAccountsPanel/ServerAccountsPanel.tsx";
import { theme } from "../../src/theme.ts";
import "@mantine/core/styles.css";
import "../../src/style.css";
import "../../src/appearance.css";

const models = observableFixture();
const areas = conversationActionsFixture(undefined, models.store);
declare global {
  interface Window {
    setTheme: (value: "dark" | "light") => void;
    publishAccounts: (value: AccountsState) => void;
  }
}
window.publishAccounts = (value) =>
  models.apply([{ collection: "accounts", id: "current", value }]);
window.publishAccounts(await (await fetch("/fixture-data")).json());

const Fixture = observer(function Fixture() {
  const [color, setColor] = useState<"dark" | "light">("dark");
  const [command, setCommand] = useState<Extract<
    ServerCommand,
    { action: "account-action" | "account-sign-in" }
  > | null>(null);
  const [error, setError] = useState("");
  window.setTheme = setColor;
  const accounts = areas.accounts;
  const server = { id: "local", origin: location.origin, label: "Fixture" };
  return (
    <MantineProvider theme={theme} forceColorScheme={color}>
      <main
        style={{ padding: 16, width: "100%", maxWidth: 760, overflow: "auto" }}
      >
        <Accounts
          state={accounts}
          scope="fixture"
          managerOnly
          onError={setError}
          externalCommand={command}
          onExternalCommandHandled={() => setCommand(null)}
        />
        {accounts && (
          <ServerAccountsPanel
            servers={[server]}
            accountsByServer={{ local: frameAccounts(accounts) }}
            statuses={{ local: "live" }}
            startAccount={(_id, value) => setCommand(value)}
            accountAction={(_id, value) => setCommand(value)}
          />
        )}
        {error && <p role="alert">{error}</p>}
      </main>
    </MantineProvider>
  );
});
createRoot(document.getElementById("root")!).render(
  <models.Provider>
    <Fixture />
  </models.Provider>,
);

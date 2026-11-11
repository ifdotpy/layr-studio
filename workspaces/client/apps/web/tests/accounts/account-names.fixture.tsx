import "../../src/rendererStyles.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { AccountsState } from "../../src/components/accounts/support/Accounts.tsx";
import { browserAreas } from "../fixtures/owner-support.tsx";
import { ServerAreasContextProvider as Provider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useState } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { MantineProvider } from "@mantine/core";
import Accounts, {
  frameAccounts,
} from "../../src/components/accounts/Accounts/Accounts.tsx";
import ServerAccountsPanel from "../../src/components/servers/ServerAccountsPanel/ServerAccountsPanel.tsx";
import { accountRows } from "../../src/servers/AccountsPanelState.ts";
import { theme } from "../../src/theme.ts";
import "@mantine/core/styles.css";
import "../../src/style.css";
import "../../src/appearance.css";

const models = observableFixture();
const areas = browserAreas(models.store);
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
  window.setTheme = setColor;
  const accounts = areas.accounts;
  const server = { id: "local", origin: location.origin, label: "Fixture" };
  return (
    <MantineProvider theme={theme} forceColorScheme={color}>
      <main
        style={{ padding: 16, width: "100%", maxWidth: 760, overflow: "auto" }}
      >
        <Accounts />
        {accounts && (
          <ServerAccountsPanel
            servers={[server]}
            rows={accountRows(
              {
                servers: [server],
                accountsByServer: { local: frameAccounts(accounts) },
                statuses: { local: "live" },
                startAccount: (_id, value) => areas.accountsUi.receive(value),
                accountAction: (_id, value) => areas.accountsUi.receive(value),
              },
              models.store,
            )}
            startAccount={(_id, value) => areas.accountsUi.receive(value)}
          />
        )}
      </main>
    </MantineProvider>
  );
});
createRoot(document.getElementById("root")!).render(
  <models.Provider>
    <Provider value={areas}>
      <Fixture />
    </Provider>
  </models.Provider>,
);

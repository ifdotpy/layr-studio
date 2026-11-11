import "../../src/rendererStyles.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type {
  Account,
  AccountsState,
} from "../../src/components/accounts/support/Accounts.tsx";
declare global {
  interface Window {
    signInAccount: string;
    publishClaudeReceipt(
      value: import("../../src/api.ts").PostResult<"/api/accounts/claude/login">,
    ): void;
  }
}
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider, Modal, Button } from "@mantine/core";
import "@mantine/core/styles.css";
import { observer } from "mobx-react-lite";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events.ts";
import { createServerStore } from "../../src/store/server.ts";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { browserAreas } from "../fixtures/owner-support.tsx";
import { Provider } from "../../src/app/ServerAreasProvider.tsx";
import ServerAccountsPanel from "../../src/components/servers/ServerAccountsPanel/ServerAccountsPanel.tsx";
import { frameAccounts } from "../../src/components/accounts/Accounts/Accounts.tsx";
import Accounts from "../../src/components/accounts/Accounts/Accounts.tsx";
import AccountSignInNotice from "../../src/components/accounts/AccountSignInNotice/AccountSignInNotice.tsx";
const account = {
  home: "/fixture/claude",
  source: "fixture",
  id: "claude",
  label: "Personal",
  email: "me@example.com",
  provider: "claude",
  status: "changed",
  error:
    "This profile's account changed. Restore its original login or add a separate profile.",
} satisfies Account;
const state: AccountsState = {
  accounts: [account],
  archivedAccounts: [],
  defaultAccountKey: "claude",
  defaultAccountGroups: [],
  supportsDelete: true,
  supportsDisconnect: true,
};
const fixture = observableFixture();
fixture.apply([{ collection: "accounts", id: "current", value: state }]);
const transient = createServerStore(defaultSubscriptions);
const store = { ...transient, state: fixture.store.state };
const areas = browserAreas(store);
window.publishClaudeReceipt = (value) =>
  defaultSubscriptions.publish(
    { kind: "auth-progress", requestId: value.requestId },
    value,
  );
const Fixture = observer(function Fixture() {
  const [opened, setOpened] = useState(false);
  const server = { id: "fixture", origin: location.origin, label: "Fixture" };
  return (
    <MantineProvider>
      <Accounts />
      <AccountSignInNotice
        account={account}
        errors={[{ message: "An unrelated earlier error" }]}
        onSignIn={(key) => (window.signInAccount = key)}
      />
      <Button onClick={() => setOpened(true)}>Manage accounts</Button>
      <Modal opened={opened} onClose={() => setOpened(false)} title="Accounts">
        <ServerAccountsPanel
          servers={[server]}
          accountsByServer={{ fixture: frameAccounts(state) }}
          statuses={{ fixture: "live" }}
          startAccount={(_id, value) => areas.accountsUi.receive(value)}
          accountAction={(_id, value) => areas.accountsUi.receive(value)}
        />
      </Modal>
    </MantineProvider>
  );
});
createRoot(document.getElementById("root")!).render(
  <ServerStoreProvider value={store}>
    <Provider value={areas}>
      <Fixture />
    </Provider>
  </ServerStoreProvider>,
);

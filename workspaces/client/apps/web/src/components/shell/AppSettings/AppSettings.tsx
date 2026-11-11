import { lazy, Suspense, type ComponentType } from "react";
import { Tabs } from "@mantine/core";
import Accounts from "../../accounts/Accounts/Accounts.tsx";
import ServerAccountsPanel from "../../servers/ServerAccountsPanel/ServerAccountsPanel.tsx";
import ServerAccessSettings from "../../servers/ServerAccessSettings/ServerAccessSettings.tsx";
import { useServerManagementPanel } from "../../../servers/ServerSettingsContext.tsx";
import { ChatSettingsDialog } from "../../agents/ChatSettingsDialog/ChatSettingsDialog.tsx";
import { StudioSettingsLayout } from "../../settings/StudioSettingsLayout/StudioSettingsLayout.tsx";
import { modalSizes } from "../../../theme.ts";
import {
  AppearanceSettings,
  HotkeySettings,
} from "../../settings/HotkeySettings/HotkeySettings.tsx";
import { Button, Modal } from "@mantine/core";
import { errorText } from "../../../api.ts";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
const MaintenanceSettings = observer(function MaintenanceSettings() {
  const { delivery } = useServerAreas();
  return (
    <section className="settings-group" aria-label="Maintenance">
      <h2>Maintenance</h2>
      <Button
        loading={delivery.removingAllSending}
        onClick={delivery.removeAllSending}
      >
        Remove all sending messages
      </Button>
    </section>
  );
});

function lazyView<P>(load: () => Promise<ComponentType<P>>) {
  return lazy(() => load().then((component) => ({ default: component })));
}
const Models = lazyView(() =>
  import("../../settings/models/ModelWhitelistSettings/ModelWhitelistSettings.tsx").then(
    (module) => module.ModelWhitelistSettings,
  ),
);

const Orchestration = lazyView(() =>
  import("../../settings/OrchestrationSettings/OrchestrationSettings.tsx").then(
    (module) => module.OrchestrationSettings,
  ),
);

const Worktrees = lazyView(() =>
  import("../../settings/LayrWorktreesSettings/LayrWorktreesSettings.tsx").then(
    (module) => module.LayrWorktreesSettings,
  ),
);

const Alerts = lazyView(() =>
  import("../../accounts/SignInPushSettings/SignInPushSettings.tsx").then(
    (module) => module.SignInPushSettings,
  ),
);

const Federation = lazyView(() =>
  import("../../settings/FederationSettings/FederationSettings.tsx").then(
    (module) => module.FederationSettings,
  ),
);

export const AppSettings = observer(function AppSettings() {
  reportPromptComposerRender("settings");
  const a = useServerAreas(),
    management = useServerManagementPanel(),
    p = a.preferenceControls;
  const lead = a.navigation.selectedLead || a.navigation.selectedAgent;
  const server = a.scope.services.owner;
  return (
    <>
      <Accounts />
      <Modal
        opened={a.settings.studioOpen && !a.accountsUi.dialogOpen}
        transitionProps={{ duration: 0 }}
        closeOnEscape={!a.accountsUi.dialogOpen}
        closeOnClickOutside={!a.accountsUi.dialogOpen}
        onClose={() => a.settings.setStudioOpen(false)}
        size={modalSizes.settings}
        title="Studio settings"
        classNames={{ body: "studio-settings-body" }}
      >
        <div className="studio-settings-panel" data-testid="studio-settings">
          <StudioSettingsLayout
            value={a.settings.tab}
            onChange={a.settings.activateTab}
          >
            <Tabs.Panel value="accounts" pt="md">
              <section className="settings-group" aria-label="Studio accounts">
                {a.accounts ? (
                  <ServerAccountsPanel
                    servers={[server]}
                    rows={a.ui.accountRows}
                    startAccount={(_id, command) =>
                      a.settings.setCommand(command)
                    }
                    onDiscover={() => {
                      void a.scope.services.api
                        .post("/api/accounts/discover", {})
                        .catch((error) => a.notices.notify(errorText(error)));
                    }}
                  />
                ) : (
                  <p role="status">Loading accounts…</p>
                )}
              </section>
            </Tabs.Panel>
            <Tabs.Panel value="servers" pt="md" keepMounted={false}>
              {management || (
                <ServerAccessSettings active={a.settings.studioOpen} />
              )}
            </Tabs.Panel>
            <Tabs.Panel
              value="models"
              className="model-whitelist-panel"
              pt="md"
              keepMounted={false}
            >
              <Suspense fallback={null}>
                <Models />
              </Suspense>
            </Tabs.Panel>
            <Tabs.Panel value="appearance" pt="md">
              <AppearanceSettings p={p}>
                <MaintenanceSettings />
              </AppearanceSettings>
            </Tabs.Panel>
            <Suspense fallback={null}>
              <Tabs.Panel value="federation" pt="md">
                {lead?.isLead && (
                  <Federation
                    active={a.settings.studioOpen}
                    leadId={lead.id}
                    agents={a.ui.catalog.agents}
                    notify={a.notices.notify}
                  />
                )}
              </Tabs.Panel>
              <Tabs.Panel value="agents" pt="md">
                <Orchestration
                  active={a.settings.studioOpen && a.settings.tab === "agents"}
                />
              </Tabs.Panel>
              <Tabs.Panel value="layr-worktrees" pt="md">
                <Worktrees active={a.settings.studioOpen} />
              </Tabs.Panel>
              <Tabs.Panel value="phone-alerts" pt="md" keepMounted={false}>
                <Alerts active={a.settings.studioOpen} />
              </Tabs.Panel>
            </Suspense>
            <Tabs.Panel value="hotkeys" pt="md">
              <HotkeySettings p={p} />
            </Tabs.Panel>
          </StudioSettingsLayout>
          {p.preferenceError && (
            <p className="studio-preferences-error" role="alert">
              {p.preferenceError}
            </p>
          )}
        </div>
      </Modal>
      <ChatSettingsDialog
        areas={a}
        settings={a.settings}
        accounts={a.accounts}
        onSignIn={a.ui.signIn}
        onNewChat={(path) => {
          void a.ui.run({ action: "new-chat", ...(path ? { path } : {}) });
        }}
        onChangeProject={a.ui.changeProjectFolder}
      />
    </>
  );
});

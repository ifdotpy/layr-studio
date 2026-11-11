import { Modal, Tabs } from "@mantine/core";
import { ShellManagement } from "../ShellManagement/ShellManagement.tsx";
import { observer } from "mobx-react-lite";
import { modalSizes } from "../../../theme.ts";
import { StudioSettingsLayout } from "../../settings/StudioSettingsLayout/StudioSettingsLayout.tsx";
import ServerAccountsPanel from "../ServerAccountsPanel/ServerAccountsPanel.tsx";
import { ShellState } from "../../../app/shell/ShellState.ts";

export const ShellSettings = observer(function ShellSettings({
  shell,
}: {
  shell: ShellState;
}) {
  const discovery = shell.discovery;
  return (
    <Modal
      opened={shell.settingsOpen}
      transitionProps={{ duration: 0 }}
      onClose={() => {
        shell.restoreAccountActionFocus = null;
        shell.set({ settingsOpen: false });
      }}
      size={modalSizes.settings}
      title="Studio settings"
      classNames={{ body: "studio-settings-body" }}
    >
      <div className="studio-settings-panel" data-testid="studio-settings">
        <StudioSettingsLayout
          value={shell.settingsTab}
          initialDetail={!!shell.restoreAccountActionFocus}
          onChange={shell.selectSettingsTab}
        >
          <Tabs.Panel value="accounts" pt="md">
            {discovery.error && <p role="alert">{discovery.error}</p>}
            <ServerAccountsPanel
              rows={shell.accountRows}
              initialFocusIdentity={
                shell.restoreAccountActionFocus || undefined
              }
              servers={shell.accountsServers}
              startAccount={shell.startAccount}
            />
          </Tabs.Panel>
          <Tabs.Panel value="servers" pt="md">
            {<ShellManagement shell={shell} />}
          </Tabs.Panel>
        </StudioSettingsLayout>
      </div>
    </Modal>
  );
});

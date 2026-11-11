import { Accordion, Modal, Select } from "@mantine/core";
import { RefreshCw } from "lucide-react";
import { observer } from "mobx-react-lite";
import type { ServerCommand } from "../../../servers/navigation.ts";
import type { StudioServer } from "../../../servers/registry.ts";
import {
  AccountAddActionsContext,
  AccountAddForm,
} from "../../accounts/AccountAddForm/AccountAddForm.tsx";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { SettingsSection } from "../../ui/PanelHeader/PanelHeader.tsx";
import { useMenuEscapeCapture } from "../../ui/support/closeMenuFromTriggerOnEscape.ts";
import { AccountRow } from "../AccountRow/AccountRow.tsx";
import {
  AccountsPanelState,
  type AccountRowView,
} from "../../../servers/AccountsPanelState.ts";
export { accountSources } from "../../../servers/AccountsPanelState.ts";

type StartAccount = (
  serverId: string,
  command: Extract<ServerCommand, { action: "account-sign-in" }>,
) => void;

export type ServerAccountsPanelProps = {
  rows: AccountRowView[];
  servers: StudioServer[];
  startAccount: StartAccount;
  onDiscover?: () => void;
  initialFocusIdentity?: string | undefined;
};

const ServerAccountsPanel = observer(function ServerAccountsPanel(
  props: ServerAccountsPanelProps,
) {
  return (
    <Owned create={() => new AccountsPanelState()}>
      {(ui) => <AccountsView {...props} ui={ui} />}
    </Owned>
  );
});
const AccountsView = observer(function AccountsView(
  props: ServerAccountsPanelProps & {
    ui: AccountsPanelState;
    rows: AccountRowView[];
  },
) {
  const { servers, onDiscover, startAccount, ui, rows } = props;
  const { draft, adding, serverId, setAdding, setServerId } = ui;
  useMenuEscapeCapture(ui.escape.openedKey !== null, ui.escape);
  const selected =
    servers.find((server) => server.id === serverId) || servers[0];
  return (
    <div className="server-accounts-panel" role="region" aria-label="Accounts">
      <SettingsSection
        title="Accounts"
        action={
          <div className="server-account-actions">
            {onDiscover && (
              <ActionButton
                actionRole="secondary"
                leftSection={<RefreshCw size={14} />}
                onClick={onDiscover}
              >
                Find existing accounts
              </ActionButton>
            )}
            <ActionButton actionRole="primary" onClick={ui.openAdd}>
              Add account
            </ActionButton>
          </div>
        }
      >
        {!rows.length ? (
          <p>No accounts found. Add a Codex or Claude account.</p>
        ) : (
          <Accordion
            className="server-accounts-list"
            keepMounted={false}
            transitionDuration={0}
            chevronPosition="left"
          >
            {props.rows.map((view) => (
              <AccountRow
                key={view.row.key}
                view={view}
                focus={props.initialFocusIdentity}
                ui={ui}
              />
            ))}
          </Accordion>
        )}
      </SettingsSection>
      <Modal
        opened={adding}
        onClose={() => setAdding(false)}
        title="Add account"
        centered
      >
        <AccountAddActionsContext
          value={{
            onCancel: () => setAdding(false),
            unavailable: !selected,
            onContinue: () => ui.continueAdd(selected, startAccount),
          }}
        >
          <AccountAddForm model={draft} gap="sm">
            <Select
              label="Server"
              data={servers.map((server) => ({
                value: server.id,
                label: server.label,
              }))}
              value={selected?.id || null}
              onChange={(value) => setServerId(value)}
            />
          </AccountAddForm>
        </AccountAddActionsContext>
      </Modal>
    </div>
  );
});
export default ServerAccountsPanel;

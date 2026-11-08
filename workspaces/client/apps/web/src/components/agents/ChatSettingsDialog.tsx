import { agentId } from "../../generated/observableResources.ts";
import { resourceData } from "../../store/runtime.ts";
import { workerCatalog } from "./WorkerModelPicker.tsx";
import { observer } from "mobx-react-lite";

import { AgentSource } from "../../generated/api";
import { ProviderKind } from "../../generated/providerKind.ts";
import { lazy, Suspense } from "react";
import { Modal, Stack, Group, Button } from "@mantine/core";
import { DelegationSettings, ExecutionPermissions } from "./ExecutionSettings";
import SubagentConcurrencyControl from "./SubagentConcurrencyControl";
import type { ServerAreas } from "../../app/contracts.ts";
import type { AccountsController } from "../Accounts.tsx";
import type { SettingsUi } from "../../app/settings/useAppSettings.ts";
import "./chat-settings-dialog.css";

const ClaudeSettings = lazy(() =>
  import("../ClaudeSettings").then((module) => ({
    default: module.ClaudeSettings,
  })),
);

export const ChatSettingsDialog = observer(function ChatSettingsDialog({
  areas,
  accounts,
  settings,
  onSignIn,
  onNewChat,
  onChangeProject,
}: {
  areas: ServerAreas;
  accounts: AccountsController;
  settings: Pick<
    SettingsUi,
    "chatOpen" | "accountModalOpen" | "closeChatSettings"
  >;
  onSignIn: (key: string) => void;
  onNewChat?: (path: string | undefined) => void;
  onChangeProject?: (
    agent: NonNullable<ServerAreas["navigation"]["selectedAgent"]>,
  ) => void;
}) {
  const { store, services } = areas.scope;
  const agent = areas.navigation.selectedAgent;
  const root = areas.navigation.selectedLead || agent;
  const detail =
    agent && resourceData(store.agentDetail(agentId(agent.id)).current);
  const rootDetail =
    root && resourceData(store.agentDetail(agentId(root.id)).current);
  const key =
    agent?.accountKey ||
    root?.accountKey ||
    (agent ? "default" : accounts.data.defaultAccountKey) ||
    "default";
  const catalog = workerCatalog(store, services.modelReader, key);
  const account = [
    ...accounts.data.accounts,
    ...accounts.data.archivedAccounts,
  ].find((a) => a.id === key);
  return (
    <Modal
      opened={settings.chatOpen}
      onClose={settings.closeChatSettings}
      closeOnEscape={!settings.accountModalOpen}
      closeOnClickOutside={!settings.accountModalOpen}
      title="Chat settings"
      size="var(--studio-team-dialog-size)"
      fullScreen={areas.layout.isSettingsFullscreen}
      className="execution-settings-modal"
    >
      <div className="chat-execution-settings">
        <Stack gap="sm">
          {root?.source === AgentSource.managed && (
            <section aria-label="Team">
              <div className="execution-team-fields">
                <SubagentConcurrencyControl
                  lead={root}
                  stateDir={accounts.scope || ""}
                  workspaceId={areas.startup.workspaceId}
                  refresh={areas.refresh}
                />
                <DelegationSettings
                  settingsRow
                  input={{ kind: "agent", summary: root, detail: rootDetail }}
                  catalog={catalog}
                  accounts={accounts.data.accounts}
                  refresh={areas.refresh}
                />
              </div>
            </section>
          )}
          <section aria-label="Permissions">
            <Stack gap="sm" id="chat-settings-permissions">
              {agent?.source === AgentSource.managed &&
                agent.provider === ProviderKind.Codex && (
                  <ExecutionPermissions
                    settingsRow
                    input={{ kind: "agent", summary: agent, detail }}
                    catalog={catalog}
                    refresh={areas.refresh}
                  />
                )}
              {agent?.provider === ProviderKind.Claude && (
                <Suspense fallback={null}>
                  <ClaudeSettings
                    key={agent.id}
                    agent={agent}
                    account={account}
                    onSignIn={onSignIn}
                  />
                </Suspense>
              )}
            </Stack>
          </section>
        </Stack>
      </div>
      {root?.projectPath && (
        <Group justify="flex-end" gap="xs" className="execution-dialog-actions">
          <Button
            size="xs"
            variant="default"
            disabled={areas.creation.isCreating}
            onClick={() => {
              settings.closeChatSettings();
              onNewChat?.(root.projectPath || undefined);
            }}
          >
            New chat in this project
          </Button>
          <Button
            size="xs"
            variant="subtle"
            onClick={() => {
              settings.closeChatSettings();
              onChangeProject?.(root);
            }}
          >
            Choose project folder
          </Button>
        </Group>
      )}
    </Modal>
  );
});

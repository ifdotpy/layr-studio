import { useServerStore } from "../../servers/ServerServicesContext.tsx";
import { resourceData } from "../../store/runtime.ts";
import { agentId as observableAgentId } from "../../generated/observableResources.ts";
import { observer } from "mobx-react-lite";

import { AgentSource } from "../../generated/api";
import { ProviderKind } from "../../generated/providerKind.ts";
import { lazy, Suspense } from "react";
import { Button, Group, Modal, Stack } from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import type { Agent, Snapshot } from "../../types";
import type { Account, useAccounts } from "../Accounts";
import { DelegationSettings, ExecutionPermissions } from "./ExecutionSettings";
import SubagentConcurrencyControl from "./SubagentConcurrencyControl";
import type { useWorkerModels } from "./WorkerModelPicker";
import "./chat-settings-dialog.css";

const ClaudeSettings = lazy(() =>
  import("../ClaudeSettings").then((module) => ({
    default: module.ClaudeSettings,
  })),
);

type Props = {
  settingsOpen: boolean;
  accountModalOpen: boolean;
  setSettingsOpen: (value: boolean) => void;
  agent?: Agent | null | undefined;
  lead?: Agent | null | undefined;
  accounts: ReturnType<typeof useAccounts>;
  notify: (message: string) => unknown;
  agentModels: ReturnType<typeof useWorkerModels>;
  data: Snapshot;
  refresh: () => Promise<void>;
  workspaceId: string;
  creating: boolean;
  newChat: (path?: string) => unknown;
  project: () => void;
  selectedAccount?: Account | undefined;
  setClaudeLoginKey: (key: string) => void;
};

export const ChatSettingsDialog = observer(function ChatSettingsDialog(
  p: Props,
) {
  const store = useServerStore();
  const mobile = useMediaQuery("(max-width: 48em)");
  const root = p.lead || p.agent;
  const detail = p.agent
    ? resourceData(store.agentDetail(observableAgentId(p.agent.id)).current)
    : undefined;
  const rootDetail = root
    ? resourceData(store.agentDetail(observableAgentId(root.id)).current)
    : undefined;
  return (
    <Modal
      opened={p.settingsOpen}
      onClose={() => {
        if (!p.accountModalOpen) p.setSettingsOpen(false);
      }}
      closeOnEscape={!p.accountModalOpen}
      closeOnClickOutside={!p.accountModalOpen}
      title="Chat settings"
      size="var(--studio-team-dialog-size)"
      fullScreen={mobile}
      className="execution-settings-modal"
    >
      <div className="chat-execution-settings">
        <Stack gap="sm">
          {root?.source === AgentSource.managed && (
            <section aria-label="Team">
              <div className="execution-team-fields">
                <SubagentConcurrencyControl
                  lead={root}
                  stateDir={p.data.stateDir}
                  workspaceId={p.workspaceId}
                  refresh={p.refresh}
                />
                <DelegationSettings
                  settingsRow
                  input={{ kind: "agent", summary: root, detail: rootDetail }}
                  catalog={p.agentModels}
                  accounts={p.accounts.data.accounts}
                  refresh={p.refresh}
                />
              </div>
            </section>
          )}
          <section aria-label="Permissions">
            <Stack gap="sm" id="chat-settings-permissions">
              {p.agent?.source === AgentSource.managed &&
                p.agent.provider === ProviderKind.Codex && (
                  <ExecutionPermissions
                    settingsRow
                    input={{ kind: "agent", summary: p.agent, detail }}
                    catalog={p.agentModels}
                    refresh={p.refresh}
                  />
                )}
              {p.agent?.provider === ProviderKind.Claude && (
                <Suspense fallback={null}>
                  <ClaudeSettings
                    key={p.agent.id}
                    agent={p.agent}
                    account={p.selectedAccount}
                    onSignIn={(key) => {
                      p.setSettingsOpen(false);
                      p.setClaudeLoginKey(key);
                    }}
                  />
                </Suspense>
              )}
            </Stack>
          </section>
        </Stack>
      </div>
      <Group justify="flex-end" gap="xs" className="execution-dialog-actions">
        {root?.projectPath && (
          <Button
            size="xs"
            variant="default"
            disabled={p.creating}
            onClick={() => {
              p.setSettingsOpen(false);
              void p.newChat(root?.projectPath || undefined);
            }}
          >
            New chat in this project
          </Button>
        )}
        {root?.projectPath && (
          <Button
            size="xs"
            variant="subtle"
            onClick={() => {
              p.setSettingsOpen(false);
              p.project();
            }}
          >
            Choose project folder
          </Button>
        )}
      </Group>
    </Modal>
  );
});

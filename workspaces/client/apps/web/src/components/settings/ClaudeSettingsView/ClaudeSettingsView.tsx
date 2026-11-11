import type { ClaudeFileBackupsStore } from "../../conversation/ClaudeFileBackups/ClaudeFileBackups.tsx";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../store/runtime.ts";
import { agentId as observableAgentId } from "../../../generated/observableResources.ts";
import ClaudeFileBackups from "../../conversation/ClaudeFileBackups/ClaudeFileBackups.tsx";
import { useServerServices } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  ClaudeSettingsStore,
  unloadedClaudeSettings,
  permissionMode,
} from "./ClaudeSettingsStore.ts";
import { accountCanStart } from "../../../accountName.ts";
import { createPortal } from "react-dom";
import { SettingsSection } from "../../ui/PanelHeader/PanelHeader.tsx";
import { SettingsRow } from "../../ui/SettingsRow/SettingsRow.tsx";
import { type ReactNode } from "react";
import {
  Button,
  NativeSelect,
  Switch,
  TextInput,
  SegmentedControl,
  Accordion,
} from "@mantine/core";
import { busy, type Agent } from "../../../types.ts";
import { useWorkerModels } from "../../agents/support/WorkerModelPicker.tsx";
import AccountSignInNotice from "../../accounts/AccountSignInNotice/AccountSignInNotice.tsx";
import type { Account } from "../../accounts/support/Accounts.tsx";
import { requiresThinking } from "studio-claude-bridge/thinking.ts";
import "../support/claude-settings.css";
import { CLAUDE_MCP_AUTO_BACKGROUND_MS } from "../../../generated/sharedValues.ts";

export const ClaudeSettingsView = function ClaudeSettings({
  agent,
  account,
  onSignIn,
  permissionsTargetId,
  fileBackups,
  model: suppliedModel,
}: {
  model?: ClaudeSettingsStore;
  permissionsTargetId?: string;
  agent: Agent;
  account?: Account | undefined;
  onSignIn?: (accountKey: string) => void;
  fileBackups?: ClaudeFileBackupsStore | undefined;
}) {
  const store = useServerStore();
  const detail = resourceData(
    store.agentDetail(observableAgentId(agent.id)).current,
  );
  const services = useServerServices();
  const model =
    suppliedModel ??
    services?.areas.settingsModels.claude ??
    unloadedClaudeSettings;

  const permissionsTarget = permissionsTargetId
    ? document.getElementById(permissionsTargetId)
    : null;
  const catalog = useWorkerModels(
    agent.accountKey || "default",
    !account || accountCanStart(account),
  );
  const thinkingRequired = requiresThinking(agent.model || "", catalog.models);
  const permissionsLocked =
    !model.stateLoaded ||
    (!!account && !accountCanStart(account)) ||
    model.busyAction ||
    !!model.savingField;
  const locked =
    permissionsLocked ||
    (typeof agent.status === "string" && busy.has(agent.status));
  const nativeStatus = detail?.nativeStatus;
  const nativeStatusError =
    nativeStatus &&
    typeof nativeStatus === "object" &&
    "error" in nativeStatus &&
    typeof nativeStatus.error === "string"
      ? nativeStatus.error
      : undefined;

  const fieldStatus = (field: string) => (
    <div className="claude-save-status" aria-live="polite">
      {model.savingField === field
        ? "Saving…"
        : model.savedField === field
          ? model.savedLabel
          : ""}
    </div>
  );
  const changePermission = (value: string) => {
    const selected = permissionMode(value);
    if (selected)
      void model.saveSetting("Permission mode", { permissionMode: selected });
  };
  const permissionSelect = (
    <NativeSelect
      label="Permission mode"
      value={model.values.permissionMode}
      disabled={permissionsLocked}
      onChange={(event) => changePermission(event.currentTarget.value)}
      data={[
        {
          value: "default",
          label: permissionsTarget ? "Ask" : "Ask for permission",
        },
        {
          value: "acceptEdits",
          label: permissionsTarget ? "Edits" : "Allow file edits",
        },
        { value: "auto", label: "Automatic" },
        { value: "plan", label: "Plan only" },
        {
          value: "bypassPermissions",
          label: permissionsTarget ? "Full" : "Full access",
        },
      ]}
    />
  );
  return (
    <section
      className="settings-group claude-settings"
      aria-label="Claude settings"
      aria-busy={model.stateLoading}
    >
      {permissionsTarget &&
        createPortal(
          <SettingsRow label="Permissions">
            <SegmentedControl
              aria-label="Permission mode"
              value={model.values.permissionMode}
              disabled={permissionsLocked}
              data={[
                { value: "default", label: "Ask" },
                { value: "acceptEdits", label: "Edits" },
                { value: "bypassPermissions", label: "Full" },
              ]}
              onChange={changePermission}
            />
            {fieldStatus("Permission mode")}
            {model.error && <span role="alert">{model.error}</span>}
          </SettingsRow>,
          permissionsTarget,
        )}
      {permissionsTarget ? (
        permissionSelect
      ) : (
        <SettingsSection title="Permissions">
          {model.stateLoading && <p role="status">Loading Claude settings…</p>}
          {permissionSelect}
          {fieldStatus("Permission mode")}
        </SettingsSection>
      )}
      <div className="claude-optional-modes">
        {thinkingRequired ? (
          <p className="claude-setting-help">
            Thinking is always on for this model.
          </p>
        ) : (
          <Switch
            label="Extended thinking"
            description="Controls how much reasoning the model uses."
            checked={model.values.thinking}
            disabled={locked || catalog.loading || !!catalog.error}
            onChange={(event) =>
              void model.saveSetting("Extended thinking", {
                thinking: event.currentTarget.checked,
              })
            }
          />
        )}
        {fieldStatus("Extended thinking")}
      </div>
      <ClaudeDisclosure
        className="claude-advanced"
        title="More settings and actions"
        alwaysOpen={!!permissionsTarget}
      >
        <div className="claude-advanced-content">
          <Switch
            label="Move slow MCP calls to the background"
            description={`Eligible Model Context Protocol (MCP) calls continue in the background after ${CLAUDE_MCP_AUTO_BACKGROUND_MS / 1000} seconds.`}
            checked={model.values.mcpAutoBackground}
            disabled={locked}
            onChange={(event) =>
              void model.saveSetting("MCP background", {
                mcpAutoBackground: event.currentTarget.checked,
              })
            }
          />
          {fieldStatus("MCP background")}
          <TextInput
            label="Auto-compact token limit"
            description="Blank uses the Claude default."
            placeholder="Claude default"
            value={model.window}
            disabled={locked}
            onChange={(event) => model.editWindow(event.currentTarget.value)}
            onBlur={() => model.saveWindow()}
            onKeyDown={(event) => event.key === "Enter" && model.saveWindow()}
          />
          {fieldStatus("Auto-compact limit")}
          <ClaudeDisclosure title="Commands and skills">
            <Button disabled={model.busyAction} onClick={model.readCommands}>
              Load commands
            </Button>
            <NativeSelect
              label="Command"
              value={model.command}
              onChange={(event) => model.setCommand(event.currentTarget.value)}
              data={[
                { value: "", label: "Select a command" },
                ...model.commands.map((item) => ({
                  value: "/" + item.name,
                  label: "/" + item.name + " " + (item.description || ""),
                })),
              ]}
            />
            <TextInput
              label="Command and arguments"
              value={model.command}
              onChange={(event) => model.setCommand(event.currentTarget.value)}
            />
            <Button
              disabled={model.busyAction || !model.command}
              onClick={() => void model.sendCommand(model.command)}
            >
              Send command
            </Button>
            <Button
              disabled={locked}
              onClick={() => void model.sendCommand("/compact", true)}
            >
              Compact conversation
            </Button>
          </ClaudeDisclosure>
          <p className="claude-version">
            Claude Code {model.state.version || "version unavailable"}
          </p>
          {(model.state.tasks || []).map((item) => (
            <div key={item.task_id} className="claude-task">
              <span>{item.description || item.task_id}</span>
              <Button
                disabled={model.busyAction}
                onClick={() => void model.stopTask(item.task_id)}
              >
                Stop task
              </Button>
            </div>
          ))}
        </div>
      </ClaudeDisclosure>
      {model.error && (
        <p role="alert" className="claude-setting-error">
          {model.error}
        </p>
      )}
      {fileBackups && model.error.includes("backup limit") && (
        <ClaudeFileBackups
          store={fileBackups}
          onReleased={() => model.setError("")}
        />
      )}
      {onSignIn && (
        <AccountSignInNotice
          account={account}
          errors={[
            model.error,
            typeof detail?.error === "string" ? detail?.error : undefined,
            nativeStatusError,
          ]}
          onSignIn={onSignIn}
        />
      )}
      {!model.stateLoaded && model.error && (
        <Button onClick={model.load}>Retry Claude settings</Button>
      )}
    </section>
  );
};

function ClaudeDisclosure({
  title,
  children,
  className = "",
  alwaysOpen = false,
}: {
  title: string;
  children: ReactNode;
  className?: string;
  alwaysOpen?: boolean;
}) {
  if (alwaysOpen) return <div className={className}>{children}</div>;
  return (
    <Accordion className={`claude-disclosure ${className}`} keepMounted={false}>
      <Accordion.Item value="settings">
        <Accordion.Control>{title}</Accordion.Control>
        <Accordion.Panel>
          <div className="claude-disclosure-content">{children}</div>
        </Accordion.Panel>
      </Accordion.Item>
    </Accordion>
  );
}

export { ClaudeSettingsStore } from "./ClaudeSettingsStore.ts";

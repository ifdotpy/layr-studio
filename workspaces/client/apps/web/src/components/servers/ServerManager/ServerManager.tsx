import { flowResult } from "mobx";
import { observer } from "mobx-react-lite";
import { localDateTime } from "../../../local-time.ts";
import { relativeSeen, serverRows } from "../../../servers/serverRows.ts";
import type { ServerDiscovery } from "../../../servers/useServerDiscovery.ts";
import { textField } from "../../../stores/fields.ts";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { SettingsSection } from "../../ui/PanelHeader/PanelHeader.tsx";
import DiscoveryPanel from "../DiscoveryPanel/DiscoveryPanel.tsx";
import ServerAccessSettings from "../ServerAccessSettings/ServerAccessSettings.tsx";
import { PairingForm } from "./PairingForm.ts";

import {
  Badge,
  CopyButton,
  DataList,
  Group,
  Modal,
  Text,
  TextInput,
  Textarea,
} from "@mantine/core";
import { Check, Copy } from "lucide-react";
import { type StudioServer } from "../../../servers/registry.ts";
import ServerDetailsEditor from "../ServerDetailsEditor/ServerDetailsEditor.tsx";

import type {} from "../../../servers/navigation.ts";
export type ServerManagerProps = {
  rows: ReturnType<typeof serverRows>;
  servers: StudioServer[];
  add: (server: StudioServer) => void;
  remove: (server: StudioServer) => void;
  discovery: Pick<
    ServerDiscovery,
    | "snapshot"
    | "checkedAt"
    | "error"
    | "busy"
    | "actions"
    | "setName"
    | "setAlias"
    | "revoke"
    | "allowAgain"
    | "retry"
  >;
  localEnabled: boolean;
};
export default observer(function ServerManager({
  servers,
  add,
  remove,
  discovery,
  localEnabled,
  rows,
}: ServerManagerProps) {
  return (
    <Owned create={() => new PairingForm()}>
      {(form) => {
        const { busy } = form;
        return (
          <div
            className="studio-settings-panel server-settings"
            role="region"
            aria-label="Servers"
          >
            <SettingsSection
              title="Servers"
              action={
                <ActionButton
                  actionRole="secondary"
                  onClick={() => form.setManual(!form.manual)}
                >
                  {form.manual
                    ? "Hide manual pairing"
                    : "Add with an invitation"}
                </ActionButton>
              }
            >
              {localEnabled && (
                <DiscoveryPanel
                  snapshot={discovery.snapshot}
                  busy={discovery.busy}
                  error={discovery.error}
                  actions={discovery.actions}
                />
              )}
              {discovery.retry && (
                <ActionButton
                  actionRole="secondary"
                  onClick={discovery.retry}
                  disabled={discovery.busy}
                >
                  Retry saved request
                </ActionButton>
              )}
              {localEnabled && !!discovery.snapshot?.identityAlerts?.length && (
                <SettingsSection title="Server identity changes">
                  {discovery.snapshot.identityAlerts.map((alert) => (
                    <Group
                      key={`${alert.oldServerId}:${alert.newServerId}`}
                      justify="space-between"
                      wrap="nowrap"
                      role="alert"
                    >
                      <Text size="sm">{alert.detail}</Text>
                    </Group>
                  ))}
                </SettingsSection>
              )}

              <div className="server-card-grid">
                {rows.map((card) => {
                  const {
                    server,
                    peer,
                    registered,
                    seen,
                    name,
                    alias,
                    state,
                    codexCount,
                    claudeCount,
                    address,
                  } = card;
                  return (
                    <article
                      key={server.id}
                      data-settings-server={server.id}
                      className="server-settings-card"
                      data-studio-state={state}
                    >
                      <header className="server-settings-card-header">
                        <div>
                          <h3 title={name}>{name}</h3>
                          <Badge
                            size="xs"
                            variant="default"
                            className="server-settings-alias"
                          >
                            {alias}
                          </Badge>
                          <span className="server-settings-status">
                            {card.displayStatus}
                          </span>
                        </div>
                        <div className="server-settings-seen">
                          {seen == null ? (
                            "Seen unavailable"
                          ) : (
                            <time
                              title={localDateTime(new Date(seen * 1000))}
                              dateTime={new Date(seen * 1000).toISOString()}
                            >
                              <span className="server-settings-seen-label">
                                Seen{" "}
                              </span>
                              {relativeSeen(seen)}
                            </time>
                          )}
                        </div>
                      </header>
                      <DataList
                        className="server-settings-facts"
                        orientation="vertical"
                        size="xs"
                        gap="xs"
                      >
                        <DataList.Item className="server-settings-host">
                          <DataList.ItemLabel>Host</DataList.ItemLabel>
                          <DataList.ItemValue>
                            <span className="server-settings-system">
                              Studio server
                            </span>
                            <span
                              className="server-settings-address"
                              title={address}
                            >
                              {address}
                            </span>
                            <CopyButton value={address}>
                              {({ copied, copy }) => (
                                <ActionButton
                                  size="compact-xs"
                                  onClick={copy}
                                  aria-label="Copy server address"
                                  title={
                                    copied ? "Copied" : "Copy server address"
                                  }
                                >
                                  {copied ? (
                                    <Check size={14} />
                                  ) : (
                                    <Copy size={14} />
                                  )}
                                </ActionButton>
                              )}
                            </CopyButton>
                          </DataList.ItemValue>
                        </DataList.Item>
                        <DataList.Item>
                          <DataList.ItemLabel>Accounts</DataList.ItemLabel>
                          <DataList.ItemValue className="server-settings-account-counts">
                            <span>Codex {codexCount}</span>
                            <span>Claude {claudeCount}</span>
                          </DataList.ItemValue>
                        </DataList.Item>
                        <DataList.Item>
                          <DataList.ItemLabel>
                            Agents running
                          </DataList.ItemLabel>
                          <DataList.ItemValue>
                            {card.agentsRunning}
                          </DataList.ItemValue>
                        </DataList.Item>
                      </DataList>
                      <Group
                        gap="xs"
                        justify="flex-end"
                        className="server-settings-actions"
                      >
                        {server.id !== "local" && registered && (
                          <ActionButton
                            actionRole="secondary"
                            size="xs"
                            aria-label="Remove from this UI"
                            title="Removal affects this UI. Revoke a manual UI connection in that server's settings."
                            disabled={busy}
                            onClick={() => void form.forget(registered, remove)}
                          >
                            Remove
                          </ActionButton>
                        )}
                        {card.canRevoke && peer && (
                          <ActionButton
                            actionRole="secondary"
                            color="red"
                            size="xs"
                            disabled={busy || discovery.busy}
                            onClick={() =>
                              form.setRevoke({
                                id: peer.id,
                                label: peer.label,
                              })
                            }
                          >
                            Revoke
                          </ActionButton>
                        )}
                        {card.canAllow && peer && (
                          <ActionButton
                            actionRole="secondary"
                            size="xs"
                            disabled={busy || discovery.busy}
                            onClick={() => discovery.allowAgain(peer.id)}
                          >
                            Allow again
                          </ActionButton>
                        )}
                      </Group>
                      {card.editable && (
                        <ServerDetailsEditor
                          name={name}
                          alias={alias}
                          usedAliases={card.usedAliases}
                          disabled={busy || discovery.busy}
                          saveName={
                            card.canSaveName
                              ? (value) => discovery.setName(server.id, value)
                              : undefined
                          }
                          saveAlias={(value) =>
                            flowResult(
                              form.saveAlias(
                                localEnabled,
                                discovery.setAlias,
                                server,
                                !!peer?.paired,
                                value,
                              ),
                            )
                          }
                        />
                      )}
                    </article>
                  );
                })}
              </div>
            </SettingsSection>
            {form.error && (
              <p className="studio-preferences-error" role="alert">
                {form.error}
              </p>
            )}
            <section
              className="server-settings-pairing"
              aria-label="Manual pairing"
            >
              {form.manual && (
                <>
                  {servers.some((server) => server.id === "local") && (
                    <ServerAccessSettings active />
                  )}
                  <p>
                    Open the server settings on the other computer. Create a
                    pairing invitation there.
                  </p>
                  <form
                    onSubmit={(event) => {
                      event.preventDefault();
                      void form.pair(add);
                    }}
                  >
                    <TextInput
                      label="Server address"
                      {...textField(form, "origin")}

                      required
                      disabled={busy}
                    />
                    <Textarea
                      label="Pairing invitation"
                      {...textField(form, "code")}

                      required
                      disabled={busy}
                      autoComplete="off"
                    />
                    <TextInput
                      label="Server name"
                      {...textField(form, "label")}

                      maxLength={128}
                      disabled={busy}
                    />
                    <ActionButton
                      type="submit"
                      actionRole="primary"
                      loading={busy}
                    >
                      Pair server
                    </ActionButton>
                  </form>
                </>
              )}
            </section>
            <Modal
              opened={!!form.revoke}
              onClose={() => form.setRevoke(null)}
              title="Revoke server access"
              size="sm"
            >
              <p>
                Revoke {form.revoke?.label} on this server? Automatic discovery
                cannot restore revoked access.
              </p>
              <Group justify="flex-end">
                <ActionButton
                  actionRole="secondary"
                  onClick={() => form.setRevoke(null)}
                >
                  Cancel
                </ActionButton>
                <ActionButton
                  actionRole="destructive"
                  onClick={() => {
                    if (form.revoke) discovery.revoke(form.revoke.id);
                    form.setRevoke(null);
                  }}
                >
                  Revoke
                </ActionButton>
              </Group>
            </Modal>
          </div>
        );
      }}
    </Owned>
  );
});

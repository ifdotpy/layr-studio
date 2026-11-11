import { useState } from "react";
import { observer } from "mobx-react-lite";
import { PeerStatus, WhoisStatus, RoomStatus } from "../../../generated/api.ts";
import { useServerServices } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button, Checkbox, Switch, Textarea, TextInput } from "@mantine/core";
import type { Agent } from "../../../types.ts";

import { FederationSettingsStore } from "./FederationSettingsStore.ts";
export { FederationSettingsStore } from "./FederationSettingsStore.ts";

const unloadedFederation = new FederationSettingsStore(null, () => {});

export const FederationSettings = observer(function FederationSettings({
  active,
  leadId,
  agents,
  notify,
  model: suppliedModel,
}: {
  model?: FederationSettingsStore;
  active: boolean;
  leadId: string;
  agents: Agent[];
  notify: (message: string) => void;
}) {
  const services = useServerServices();
  const model =
    suppliedModel ??
    services?.areas.settingsModels.federation ??
    unloadedFederation;

  const [inviteText, setInviteText] = useState("");
  const [serverLabel, setServerLabel] = useState("Studio server");
  const [expectedUser, setExpectedUser] = useState("");
  const [shareNames, setShareNames] = useState(false);
  const [shareStatus, setShareStatus] = useState(false);
  if (!active) return null;
  const { state, busy, run } = model;
  const localMembers = model.members(leadId);
  const team = agents.filter(
    (agent) => agent.id === leadId || agent.rootId === leadId,
  );

  const members = (
    <>
      {team.map((agent) => (
        <Checkbox
          key={agent.id}
          label={agent.isLead ? `${agent.name} (lead)` : agent.name}
          checked={localMembers.includes(agent.id)}
          disabled={busy || (agent.id !== leadId && !shareNames)}
          onChange={(event) =>
            model.toggleMember(leadId, agent.id, event.currentTarget.checked)
          }
        />
      ))}
      <Checkbox
        label="Share names of selected agents"
        checked={shareNames}
        onChange={(event) => setShareNames(event.currentTarget.checked)}
      />
      <Checkbox
        label="Share selected agents’ current status"
        checked={shareStatus}
        onChange={(event) => setShareStatus(event.currentTarget.checked)}
      />
    </>
  );

  return (
    <section
      className="settings-group federation-settings"
      aria-label="Peer federation"
    >
      <h2>Peer federation</h2>
      <p>Share rooms with another Studio.</p>
      <Switch
        label="Enable federation traffic on this server"
        checked={state?.enabled === true}
        disabled={busy || !state}
        onChange={(event) =>
          void run({
            action: "set_enabled",
            enabled: event.currentTarget.checked,
          })
        }
      />
      {state?.identity && (
        <p className="federation-identity">
          Server identity: {state.identity.label} · key{" "}
          {state.identity.fingerprint}
        </p>
      )}
      <section className="federation-invitation">
        <h3>Create an invitation</h3>
        <TextInput
          label="This server's label"
          value={serverLabel}
          onChange={(event) => setServerLabel(event.currentTarget.value)}
          maxLength={80}
        />
        <TextInput
          label="Expected Tailscale user (optional)"
          description="A mismatch rejects pairing. A missing identity requires explicit acceptance."
          value={expectedUser}
          onChange={(event) => setExpectedUser(event.currentTarget.value)}
          maxLength={254}
        />
        <Button
          variant="light"
          disabled={busy || !state?.enabled}
          onClick={() =>
            void model.invite(serverLabel, expectedUser, setInviteText)
          }
        >
          Create pairing invitation
        </Button>
      </section>
      <section className="federation-invitation">
        <h3>Accept an invitation</h3>
        <p>
          Create your invitation and exchange it privately with the other user.
          Paste their invitation below. It expires after 15 minutes.
        </p>
        <Textarea
          label="Exchange invitation JSON privately"
          value={inviteText}
          onChange={(event) => setInviteText(event.currentTarget.value)}
          minRows={4}
          autosize
          maxRows={12}
        />
        <Button
          disabled={busy || !state?.enabled || !inviteText.trim()}
          onClick={() => void model.pair(inviteText, setInviteText, notify)}
        >
          Approve pairing
        </Button>
      </section>
      {state?.peers.map((peer) => (
        <div className="federation-peer" key={peer.stateId}>
          <strong>{peer.label}</strong>
          <span>
            {peer.status === PeerStatus.approved ? "Paired" : peer.status}
          </span>
          {peer.whoisStatus === WhoisStatus.missing && (
            <p role="alert">
              Tailscale could not verify this user. Continue only if you
              recognize and accept this peer.
            </p>
          )}
          {peer.lastError && <p role="status">{peer.lastError}</p>}
          {peer.status !== PeerStatus.approved &&
            peer.status !== PeerStatus.revoked && (
              <Button
                size="xs"
                disabled={busy}
                onClick={() =>
                  void run({
                    action: "approve_peer",
                    state_id: peer.stateId,
                    accept_missing_whois:
                      peer.whoisStatus === WhoisStatus.missing,
                  })
                }
              >
                {peer.whoisStatus === WhoisStatus.missing
                  ? "Accept missing identity and pair"
                  : "Approve pairing"}
              </Button>
            )}
          {peer.status !== PeerStatus.revoked && (
            <Button
              size="xs"
              color="red"
              variant="subtle"
              disabled={busy}
              onClick={() =>
                void run({ action: "revoke_peer", state_id: peer.stateId })
              }
            >
              Revoke
            </Button>
          )}
        </div>
      ))}
      {state?.peers.some((peer) => peer.status === PeerStatus.approved) && (
        <div className="federation-room-create">
          <h3>Create a shared room</h3>
          <p>
            By default, only the local lead display name is shared. Extra names
            and status require opt-in.
          </p>
          {members}
          {state.peers
            .filter((peer) => peer.status === PeerStatus.approved)
            .map((peer) => (
              <Button
                key={peer.stateId}
                variant="light"
                disabled={
                  busy || !state.enabled || !localMembers.includes(leadId)
                }
                onClick={() =>
                  void run({
                    action: "create_room",
                    peer_id: peer.stateId,
                    local_members: localMembers,
                    share_names: shareNames,
                    share_status: shareStatus,
                  })
                }
              >
                Invite {peer.label} to a room
              </Button>
            ))}
        </div>
      )}
      {state?.rooms.map((room) => (
        <div className="federation-room" key={room.id}>
          <strong>{room.peerLabel}</strong>
          <span>
            {room.status === RoomStatus.approved
              ? "Shared room"
              : "Room approval pending"}
          </span>
          {room.status === RoomStatus.pending &&
            room.localMembers.length === 0 && (
              <>
                <p>
                  Approve access for the local lead. Select extra members only
                  when needed.
                </p>
                {members}
                <Button
                  disabled={busy || !localMembers.includes(leadId)}
                  onClick={() =>
                    void run({
                      action: "approve_room",
                      room_id: room.id,
                      local_members: localMembers,
                      share_names: shareNames,
                      share_status: shareStatus,
                    })
                  }
                >
                  Approve room
                </Button>
              </>
            )}
        </div>
      ))}
      {state && state.queued > 0 && (
        <p role="status">{state.queued} message(s) waiting for delivery.</p>
      )}
    </section>
  );
});

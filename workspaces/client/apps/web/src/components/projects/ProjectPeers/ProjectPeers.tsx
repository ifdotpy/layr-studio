import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import {
  useServerApi,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button, Checkbox, NativeSelect, Textarea } from "@mantine/core";
import { useEffect, useState } from "react";
import { errorText, type PostResult } from "../../../api.ts";
import type { components } from "../../../generated/api.ts";
import {
  capabilityLabel,
  fingerprintOf,
  formatFingerprint,
  parseProjectInvitation,
  type PeerInvitation,
} from "./peerHelpers.ts";

type ProjectGrant = components["schemas"]["ProjectGrant"];

/** The grants of a list or change reply. */
function grantsOf(result: PostResult<"/api/federation">): ProjectGrant[] {
  if (!("grants" in result))
    throw new Error("The server sent no project grants.");
  return result.grants;
}

export const ProjectPeers = observer(function ProjectPeers({
  projectId,
  notify,
}: {
  projectId: string;
  notify: (message: string) => void;
}) {
  const { post } = useServerApi();
  const store = useServerStore();
  const projects = [
    ...(resourceData(store.state().current)?.projects.values() || []),
  ];
  const [grants, setGrants] = useState<ProjectGrant[]>([]);
  const [inviteText, setInviteText] = useState("");
  const [pasted, setPasted] = useState("");
  const [invitation, setInvitation] = useState<PeerInvitation | null>(null);
  const [fingerprint, setFingerprint] = useState("");
  const [localProject, setLocalProject] = useState(projectId);
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = async () => {
    const result = await post("/api/federation", {
      action: "list_project_grants",
      project_id: projectId,
    });
    setGrants(grantsOf(result));
  };

  useEffect(() => {
    void load().catch((error) => notify(errorText(error)));
    // Reload when the project changes only.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId]);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
    } catch (error) {
      notify(errorText(error));
    } finally {
      setBusy(false);
    }
  };

  const create = () =>
    run(async () => {
      const result = await post("/api/federation", {
        action: "create_project_invite",
        project_id: projectId,
      });
      if (!("invitation" in result)) throw new Error("No invitation returned.");
      setInviteText(JSON.stringify(result.invitation, null, 2));
    });

  const choose = () =>
    run(async () => {
      const parsed = parseProjectInvitation(pasted);
      if (!parsed) throw new Error("Paste a valid project invitation.");
      setInvitation(parsed);
      setFingerprint(await fingerprintOf(parsed.publicKey));
      setConfirmed(false);
    });

  const accept = () =>
    run(async () => {
      if (!invitation) return;
      const result = await post("/api/federation", {
        action: "accept_project_peer",
        invitation,
        project_id: localProject,
      });
      setGrants(grantsOf(result));
      setInvitation(null);
      setPasted("");
    });

  const revoke = (grantId: string) =>
    run(async () => {
      const result = await post("/api/federation", {
        action: "revoke_project_grant",
        grant_id: grantId,
      });
      setGrants(grantsOf(result));
    });

  return (
    <section className="settings-group project-peers" aria-label="Peers">
      <h2>Peers</h2>
      <p>Pair this project with a project on another server.</p>

      <section>
        <h3>Create an invitation</h3>
        <Button variant="light" disabled={busy} onClick={() => void create()}>
          Create invitation
        </Button>
        {inviteText && (
          <>
            <Textarea
              label="Invitation (send it privately, expires in 15 minutes)"
              value={inviteText}
              readOnly
              autosize
              minRows={4}
              maxRows={12}
            />
            <Button
              size="xs"
              variant="subtle"
              onClick={() => void navigator.clipboard?.writeText(inviteText)}
            >
              Copy invitation
            </Button>
          </>
        )}
      </section>

      <section>
        <h3>Accept an invitation</h3>
        <Textarea
          label="Paste the invitation"
          value={pasted}
          onChange={(event) => setPasted(event.currentTarget.value)}
          autosize
          minRows={3}
          maxRows={10}
        />
        <Button
          variant="light"
          disabled={busy || !pasted.trim()}
          onClick={() => void choose()}
        >
          Check invitation
        </Button>
        {invitation && (
          <div className="project-peer-confirm">
            <p>Peer key fingerprint: {formatFingerprint(fingerprint)}</p>
            <p>Capability: {capabilityLabel("LeadMessages")}</p>
            <NativeSelect
              label="Local project"
              value={localProject}
              onChange={(event) => setLocalProject(event.currentTarget.value)}
              data={projects.map((row) => ({
                value: row.id,
                label: row.name || row.id,
              }))}
            />
            <Checkbox
              label="The fingerprint matches the other owner's fingerprint"
              checked={confirmed}
              onChange={(event) => setConfirmed(event.currentTarget.checked)}
            />
            <Button disabled={busy || !confirmed} onClick={() => void accept()}>
              Pair project
            </Button>
          </div>
        )}
      </section>

      <section>
        <h3>Paired projects</h3>
        {grants.length === 0 && <p>No paired projects.</p>}
        {grants.map((grant) => (
          <div className="project-peer" key={grant.grantId}>
            <strong>Fingerprint {formatFingerprint(grant.fingerprint)}</strong>
            <span>{grant.capabilities.map(capabilityLabel).join(", ")}</span>
            <Button
              size="xs"
              color="red"
              variant="light"
              disabled={busy}
              onClick={() => void revoke(grant.grantId)}
            >
              Revoke
            </Button>
          </div>
        ))}
      </section>
    </section>
  );
});

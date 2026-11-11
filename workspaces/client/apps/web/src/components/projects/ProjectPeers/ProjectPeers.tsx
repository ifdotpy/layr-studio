import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button, Checkbox, NativeSelect, Textarea } from "@mantine/core";
import type { createProjectPeersStore } from "./ProjectPeersStore.ts";
import { capabilityLabel, formatFingerprint } from "./peerHelpers.ts";

type Props = {
  form: ReturnType<typeof createProjectPeersStore>;
};
export const ProjectPeers = observer(function ProjectPeers({ form }: Props) {
  const store = useServerStore();
  const projects = [
    ...(resourceData(store.state().current)?.projects.values() || []),
  ];

  return (
    <section className="settings-group project-peers" aria-label="Peers">
      <h2>Peers</h2>
      <p>Pair this project with a project on another server.</p>

      <section>
        <h3>Create an invitation</h3>
        <Button
          variant="light"
          disabled={form.busy}
          onClick={() => void form.create()}
        >
          Create invitation
        </Button>
        {form.inviteText && (
          <>
            <Textarea
              label="Invitation (send it privately, expires in 15 minutes)"
              value={form.inviteText}
              readOnly
              autosize
              minRows={4}
              maxRows={12}
            />
            <Button
              size="xs"
              variant="subtle"
              onClick={() =>
                void navigator.clipboard?.writeText(form.inviteText)
              }
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
          value={form.pasted}
          onChange={(event) =>
            form.change({ pasted: event.currentTarget.value })
          }
          autosize
          minRows={3}
          maxRows={10}
        />
        <Button
          variant="light"
          disabled={form.busy || !form.pasted.trim()}
          onClick={() => void form.choose()}
        >
          Check invitation
        </Button>
        {form.invitation && (
          <div className="project-peer-confirm">
            <p>Peer key fingerprint: {formatFingerprint(form.fingerprint)}</p>
            <p>Capability: {capabilityLabel("LeadMessages")}</p>
            <NativeSelect
              label="Local project"
              value={form.localProject}
              onChange={(event) =>
                form.change({ localProject: event.currentTarget.value })
              }
              data={projects.map((row) => ({
                value: row.id,
                label: row.name || row.id,
              }))}
            />
            <Checkbox
              label="The fingerprint matches the other owner's fingerprint"
              checked={form.confirmed}
              onChange={(event) =>
                form.change({ confirmed: event.currentTarget.checked })
              }
            />
            <Button
              disabled={form.busy || !form.confirmed}
              onClick={() => void form.accept()}
            >
              Pair project
            </Button>
          </div>
        )}
      </section>

      <section>
        <h3>Paired projects</h3>
        {form.grants.length === 0 && <p>No paired projects.</p>}
        {form.grants.map((grant) => (
          <div className="project-peer" key={grant.grantId}>
            <strong>Fingerprint {formatFingerprint(grant.fingerprint)}</strong>
            <span>{grant.capabilities.map(capabilityLabel).join(", ")}</span>
            <Button
              size="xs"
              color="red"
              variant="light"
              disabled={form.busy}
              onClick={() => void form.revoke(grant.grantId)}
            >
              Revoke
            </Button>
          </div>
        ))}
      </section>
    </section>
  );
});

export { createProjectPeersStore } from "./ProjectPeersStore.ts";
